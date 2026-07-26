use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{Local, Timelike};
use openaction::global_events::{
	set_global_event_handler, DeviceDidConnectEvent, GlobalEventHandler, SystemDidWakeUpEvent,
};
use openaction::*;
use serde::{Deserialize, Serialize};

/// 时钟位：该按键实例显示 HH:MM:SS 中的哪一位（Pair 为两位同格显示）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Position {
	#[default]
	HourTens,
	HourOnes,
	HourPair,
	Colon,
	MinuteTens,
	MinuteOnes,
	MinutePair,
	SecondTens,
	SecondOnes,
	SecondPair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct DigitSettings {
	position: Position,
	twenty_four_hour: bool,
	blink_colon: bool,
	color: String,
	background: String,
	/// 按键时执行的命令，空则无动作
	command: String,
}

impl Default for DigitSettings {
	fn default() -> Self {
		Self {
			position: Position::default(),
			twenty_four_hour: true,
			blink_colon: true,
			color: "#ffffff".to_owned(),
			background: "#000000".to_owned(),
			command: String::new(),
		}
	}
}

/// 单个实例的强制刷新周期（秒），对抗设备重置等未发事件的画面丢失
const FORCE_REFRESH_PERIOD: u32 = 15;
/// 单次 tick 允许发送的 setImage 上限（熔断，防止对宿主/设备的突发洪峰）
const MAX_IMAGES_PER_TICK: u32 = 20;
/// 全量失效的最小间隔，防止设备重连抖动引发重绘风暴
const MIN_INVALIDATE_INTERVAL: Duration = Duration::from_secs(5);

/// 每个实例的当前设置（tick 任务据此渲染）
static SETTINGS: LazyLock<Mutex<HashMap<InstanceId, DigitSettings>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));
/// 每个实例上一次渲染的文本，用于跳过无变化的帧
static LAST_TEXT: LazyLock<Mutex<HashMap<InstanceId, String>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));
/// 上一次全量失效的时刻，用于防抖
static LAST_INVALIDATE: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// 计算某实例当前应显示的文本（单位或两位）
fn text_for(settings: &DigitSettings, hour: u32, minute: u32, second: u32) -> String {
	let digit = |value: u32, tens: bool| -> String {
		let d = if tens { value / 10 } else { value % 10 };
		char::from_digit(d, 10).unwrap_or('0').to_string()
	};
	let display_hour = if settings.twenty_four_hour {
		hour
	} else {
		match hour % 12 {
			0 => 12,
			h => h,
		}
	};
	match settings.position {
		// 12 小时制下时十位为 0 时留空更自然
		Position::HourTens if !settings.twenty_four_hour && display_hour < 10 => " ".to_owned(),
		Position::HourTens => digit(display_hour, true),
		Position::HourOnes => digit(display_hour, false),
		// 两位同格：12 小时制不补前导零，居中更自然
		Position::HourPair if !settings.twenty_four_hour => format!("{display_hour}"),
		Position::HourPair => format!("{display_hour:02}"),
		Position::Colon if settings.blink_colon && second % 2 == 1 => " ".to_owned(),
		Position::Colon => ":".to_owned(),
		Position::MinuteTens => digit(minute, true),
		Position::MinuteOnes => digit(minute, false),
		Position::MinutePair => format!("{minute:02}"),
		Position::SecondTens => digit(second, true),
		Position::SecondOnes => digit(second, false),
		Position::SecondPair => format!("{second:02}"),
	}
}

/// 颜色值白名单校验，防止拼进 SVG 的内容破坏结构
fn safe_color<'a>(value: &'a str, fallback: &'a str) -> &'a str {
	let valid = value.len() <= 9
		&& value.starts_with('#')
		&& value[1..].chars().all(|c| c.is_ascii_hexdigit());
	if valid {
		value
	} else {
		fallback
	}
}

/// 把文本（1-2 个字符）渲染成 144x144 SVG data URI，字号随长度自适应
fn render_svg(text: &str, settings: &DigitSettings) -> String {
	let fg = safe_color(&settings.color, "#ffffff");
	let bg = safe_color(&settings.background, "#000000");
	// 单字符大字号；两字符缩小以完整容纳
	let (font_size, baseline_y) = if text.trim().chars().count() > 1 {
		(80, 101)
	} else {
		(112, 112)
	};
	let svg = format!(
		concat!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144">"#,
			r#"<rect width="144" height="144" fill="{bg}"/>"#,
			r#"<text x="72" y="{y}" font-family="sans-serif" font-size="{size}" font-weight="bold" fill="{fg}" text-anchor="middle">{text}</text>"#,
			"</svg>"
		),
		bg = bg,
		fg = fg,
		y = baseline_y,
		size = font_size,
		text = text,
	);
	format!("data:image/svg+xml;base64,{}", BASE64.encode(svg))
}

/// 立即为单个实例渲染一帧（无视缓存），用于 willAppear / 设置变更
async fn redraw_instance(instance: &Instance, settings: &DigitSettings) -> OpenActionResult<()> {
	let now = Local::now();
	let text = text_for(settings, now.hour(), now.minute(), now.second());
	let image = render_svg(&text, settings);
	LAST_TEXT
		.lock()
		.unwrap()
		.insert(instance.instance_id.clone(), text);
	instance.set_image(Some(image), None).await
}

struct DigitAction;

#[async_trait]
impl Action for DigitAction {
	type Settings = DigitSettings;
	const UUID: &'static str = "com.gdwhisper.tikclock.digit";

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		SETTINGS
			.lock()
			.unwrap()
			.insert(instance.instance_id.clone(), settings.clone());
		redraw_instance(instance, settings).await
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		SETTINGS.lock().unwrap().remove(&instance.instance_id);
		LAST_TEXT.lock().unwrap().remove(&instance.instance_id);
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		SETTINGS
			.lock()
			.unwrap()
			.insert(instance.instance_id.clone(), settings.clone());
		redraw_instance(instance, settings).await
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let command = settings.command.trim();
		if command.is_empty() {
			return Ok(());
		}
		// 静默拉起子进程，不等待、不闪控制台窗口
		match spawn_command(command) {
			Ok(_) => Ok(()),
			Err(error) => {
				log::warn!("failed to run command {command:?}: {error}");
				instance.show_alert().await
			}
		}
	}
}

/// 用平台默认 shell 静默执行命令（Windows: cmd /C；其他: sh -c）
fn spawn_command(command: &str) -> std::io::Result<std::process::Child> {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		const CREATE_NO_WINDOW: u32 = 0x0800_0000;
		std::process::Command::new("cmd")
			.args(["/C", command])
			.creation_flags(CREATE_NO_WINDOW)
			.spawn()
	}
	#[cfg(not(windows))]
	{
		std::process::Command::new("sh")
			.args(["-c", command])
			.spawn()
	}
}

/// 强制下一次 tick 全量重绘（清空差分缓存）；带防抖，设备重连抖动时不会反复触发
fn invalidate_all() {
	let mut last = LAST_INVALIDATE.lock().unwrap();
	if let Some(at) = *last {
		if at.elapsed() < MIN_INVALIDATE_INTERVAL {
			return;
		}
	}
	*last = Some(Instant::now());
	LAST_TEXT.lock().unwrap().clear();
}

/// 实例在强制刷新周期内的相位：按 ID 哈希错峰，避免所有实例在同一 tick 集中重发
fn instance_phase(instance_id: &str) -> u32 {
	instance_id
		.bytes()
		.fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
		% FORCE_REFRESH_PERIOD
}

struct GlobalHandler;

#[async_trait]
impl GlobalEventHandler for GlobalHandler {
	// 设备重连/系统唤醒后画面可能被宿主重置，必须重发图像
	async fn device_did_connect(&self, _event: DeviceDidConnectEvent) -> OpenActionResult<()> {
		invalidate_all();
		Ok(())
	}

	async fn system_did_wake_up(&self, _event: SystemDidWakeUpEvent) -> OpenActionResult<()> {
		invalidate_all();
		Ok(())
	}
}

/// 每秒对齐整秒 tick，只重绘字符发生变化的实例；强制刷新按实例错峰，单 tick 发送有熔断
async fn tick_loop() {
	let mut ticks: u32 = 0;
	loop {
		let ms = Local::now().timestamp_subsec_millis() as u64;
		tokio::time::sleep(Duration::from_millis(1000u64.saturating_sub(ms).max(1) + 5)).await;

		ticks = ticks.wrapping_add(1);
		let now = Local::now();
		let (hour, minute, second) = (now.hour(), now.minute(), now.second());
		let mut sent: u32 = 0;

		for instance in visible_instances(DigitAction::UUID).await {
			// 计算并比对缓存；不持锁跨 await，发送成功前不更新缓存
			let pending = {
				let settings_map = SETTINGS.lock().unwrap();
				let Some(settings) = settings_map.get(&instance.instance_id) else {
					continue;
				};
				let text = text_for(settings, hour, minute, second);
				// 周期性强制重绘对抗画面丢失，相位错峰后每 tick 最多多发少量几帧
				let force = instance_phase(&instance.instance_id) == ticks % FORCE_REFRESH_PERIOD;
				let unchanged =
					LAST_TEXT.lock().unwrap().get(&instance.instance_id) == Some(&text);
				if unchanged && !force {
					None
				} else {
					let image = render_svg(&text, settings);
					Some((text, image))
				}
			};
			if let Some((text, image)) = pending {
				// 熔断：超出预算的实例不更新缓存，下个 tick 自动补发
				if sent >= MAX_IMAGES_PER_TICK {
					log::warn!("per-tick image budget exhausted, deferring {}", instance.instance_id);
					LAST_TEXT.lock().unwrap().remove(&instance.instance_id);
					continue;
				}
				sent += 1;
				LAST_TEXT
					.lock()
					.unwrap()
					.insert(instance.instance_id.clone(), text);
				if let Err(error) = instance.set_image(Some(image), None).await {
					log::warn!("setImage failed for {}: {error:?}", instance.instance_id);
				}
			}
		}
	}
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
	let _ = simplelog::SimpleLogger::init(
		simplelog::LevelFilter::Info,
		simplelog::Config::default(),
	);

	register_action(DigitAction).await;
	set_global_event_handler(&GlobalHandler);
	tokio::spawn(tick_loop());
	run(std::env::args().collect()).await
}

#[cfg(test)]
mod tests {
	use super::*;

	fn settings(position: Position) -> DigitSettings {
		DigitSettings {
			position,
			..DigitSettings::default()
		}
	}

	#[test]
	fn splits_24h_time_into_digits() {
		let cases = [
			(Position::HourTens, "0"),
			(Position::HourOnes, "9"),
			(Position::MinuteTens, "3"),
			(Position::MinuteOnes, "5"),
			(Position::SecondTens, "0"),
			(Position::SecondOnes, "7"),
		];
		for (position, expected) in cases {
			assert_eq!(text_for(&settings(position), 9, 35, 7), expected);
		}
	}

	#[test]
	fn pair_positions_show_two_digits() {
		assert_eq!(text_for(&settings(Position::HourPair), 9, 35, 7), "09");
		assert_eq!(text_for(&settings(Position::MinutePair), 9, 35, 7), "35");
		assert_eq!(text_for(&settings(Position::SecondPair), 9, 35, 7), "07");
	}

	#[test]
	fn hour_pair_in_12h_mode_drops_leading_zero() {
		let mut s = settings(Position::HourPair);
		s.twenty_four_hour = false;
		assert_eq!(text_for(&s, 21, 0, 0), "9");
		assert_eq!(text_for(&s, 0, 0, 0), "12");
		assert_eq!(text_for(&s, 12, 0, 0), "12");
	}

	#[test]
	fn twelve_hour_mode_converts_and_blanks_leading_zero() {
		let mut s = settings(Position::HourTens);
		s.twenty_four_hour = false;
		// 21:00 -> 9 点，十位留空
		assert_eq!(text_for(&s, 21, 0, 0), " ");
		s.position = Position::HourOnes;
		assert_eq!(text_for(&s, 21, 0, 0), "9");
		// 午夜 0 点 -> 12
		s.position = Position::HourTens;
		assert_eq!(text_for(&s, 0, 0, 0), "1");
		s.position = Position::HourOnes;
		assert_eq!(text_for(&s, 0, 0, 0), "2");
		// 中午 12 点保持 12
		assert_eq!(text_for(&s, 12, 0, 0), "2");
	}

	#[test]
	fn twenty_four_hour_mode_keeps_leading_zero() {
		assert_eq!(text_for(&settings(Position::HourTens), 9, 0, 0), "0");
		assert_eq!(text_for(&settings(Position::HourTens), 23, 0, 0), "2");
	}

	#[test]
	fn colon_blinks_on_odd_seconds() {
		let mut s = settings(Position::Colon);
		assert_eq!(text_for(&s, 0, 0, 0), ":");
		assert_eq!(text_for(&s, 0, 0, 1), " ");
		s.blink_colon = false;
		assert_eq!(text_for(&s, 0, 0, 1), ":");
	}

	#[test]
	fn rejects_malformed_colors() {
		assert_eq!(safe_color("#12abEF", "#000000"), "#12abEF");
		assert_eq!(safe_color("red\"/><script>", "#000000"), "#000000");
		assert_eq!(safe_color("", "#ffffff"), "#ffffff");
	}

	#[test]
	fn phase_is_deterministic_and_in_period() {
		for id in ["a", "instance-123", "com.gdwhisper.tikclock.digit.0", ""] {
			let phase = instance_phase(id);
			assert!(phase < FORCE_REFRESH_PERIOD);
			assert_eq!(phase, instance_phase(id));
		}
	}

	#[test]
	fn phases_spread_instances_across_ticks() {
		// 相邻 ID（实际实例 ID 常为递增序号）应落在不同相位，而非同一 tick 集中重发
		let phases: std::collections::HashSet<u32> = (0..15)
			.map(|i| instance_phase(&format!("instance-{i}")))
			.collect();
		assert!(phases.len() > 5, "phases too clustered: {phases:?}");
	}
}
