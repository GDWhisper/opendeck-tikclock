use std::sync::LazyLock;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};

use crate::{safe_color, DigitSettings, Position};

/// 按键外观样式：显示什么字符由 main.rs 的 text_for 决定，本模块只负责画成长什么样。
/// 新增样式 = 加一个变体 + 加一个 body_xxx 生成函数，外壳不用动。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigitStyle {
	#[default]
	Text,
	SevenSegment,
}

/// 画布边长（px）
const TILE: f64 = 144.0;
/// 单字符字身高（px）
const GLYPH_HEIGHT: f64 = 112.0;
/// 两位同格时的字身高与字距（px）
const PAIR_HEIGHT: f64 = 88.0;
const PAIR_SPACING: f64 = 16.0;
/// 七段字身归一化尺寸：宽 520、高 1000（长宽比贴近真实数码管，顶点表用整数，排版用浮点）
const BOX_W: f64 = 520.0;
const BOX_H: f64 = 1000.0;
/// 数码管倾斜角（度）
const SLANT_DEG: f64 = 8.0;
/// 暗段（未点亮段）透明度
const GHOST_OPACITY: f64 = 0.08;
/// 短横线只点亮中段 g
const DASH_SEGMENT: u8 = 1 << 6;

/// 数字 → 点亮的段，bit0..bit6 依次为 a(顶) b(右上) c(右下) d(底) e(左下) f(左上) g(中)
const SEGMENTS: [u8; 10] = [
	0b011_1111, // 0: a b c d e f
	0b000_0110, // 1: b c
	0b101_1011, // 2: a b d e g
	0b100_1111, // 3: a b c d g
	0b110_0110, // 4: b c f g
	0b110_1101, // 5: a c d f g
	0b111_1101, // 6: a c d e f g
	0b000_0111, // 7: a b c
	0b111_1111, // 8: 全亮
	0b110_1111, // 9: a b c d f g
];

/// 段粗细
const SEG_T: i32 = 90;
/// 段端缩进量。数码管的段是六根独立斜切条，交汇处只留一条缝；
/// 取 0 会让两端尖角正好对接，取负值则相互插入把角糊死（早先的样子）
const SEG_GAP: i32 = 32;

/// 横向段：两端为 45° 尖角，参数是左右尖角顶点（同一 y，即段中线）
fn hex_h(xa: i32, cy: i32, xb: i32) -> String {
	let h = SEG_T / 2;
	polygon_points(&[(xa, cy), (xa + h, cy - h), (xb - h, cy - h), (xb, cy), (xb - h, cy + h), (xa + h, cy + h)])
}

/// 竖向段：两端为 45° 尖角，参数是上下尖角顶点（同一 x，即段中线）
fn hex_v(cx: i32, ya: i32, yb: i32) -> String {
	let h = SEG_T / 2;
	polygon_points(&[(cx, ya), (cx + h, ya + h), (cx + h, yb - h), (cx, yb), (cx - h, yb - h), (cx - h, ya + h)])
}

/// 顶点列表 → SVG polygon 的 points 串
fn polygon_points(pts: &[(i32, i32)]) -> String {
	pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ")
}

/// 7 个段在字身内的顶点串，下标顺序与 SEGMENTS 的 bit 序一致（a b c d e f g）
static SEG_POLYGONS: LazyLock<[String; 7]> = LazyLock::new(|| {
	let (w, h) = (BOX_W as i32, BOX_H as i32);
	let half = SEG_T / 2;
	// 段中线：三条横线分别在顶、中、底，两条竖线分别在左右
	let (cy_top, cy_mid, cy_bot) = (half, h / 2, h - half);
	let (cx_left, cx_right) = (half, w - half);
	// 每条段的尖角都从交汇处退缩 SEG_GAP，四角只留斜缝不相连
	let (x0, x1) = (cx_left + SEG_GAP, cx_right - SEG_GAP);
	let (y_top0, y_top1) = (cy_top + SEG_GAP, cy_mid - SEG_GAP);
	let (y_bot0, y_bot1) = (cy_mid + SEG_GAP, cy_bot - SEG_GAP);
	[
		hex_h(x0, cy_top, x1), // a 顶
		hex_v(cx_right, y_top0, y_top1), // b 右上
		hex_v(cx_right, y_bot0, y_bot1), // c 右下
		hex_h(x0, cy_bot, x1), // d 底
		hex_v(cx_left, y_bot0, y_bot1), // e 左下
		hex_v(cx_left, y_top0, y_top1), // f 左上
		hex_h(x0, cy_mid, x1), // g 中
	]
});

/// 变换属性里的数值：保留 3 位小数并去掉无意义的尾随零，保持 SVG 紧凑
fn num(v: f64) -> String {
	let s = format!("{v:.3}");
	s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// 把时间或日期文本渲染成 144x144 SVG data URI
pub(crate) fn render_svg(text: &str, settings: &DigitSettings) -> String {
	let bg = safe_color(&settings.background, "#000000");
	let body = match settings.style {
		DigitStyle::Text => body_text(text, settings),
		DigitStyle::SevenSegment if text == " " && settings.ghost_segments => {
			// 闪烁熄灭时，按位置画出暗淡的分隔符。
			let separator = match settings.position {
				Position::Colon => ":",
				Position::Dash => "-",
				_ => "",
			};
			if separator.is_empty() {
				String::new()
			} else {
				let shapes = body_segments(separator, settings);
				format!(r#"<g fill-opacity="{GHOST_OPACITY}">{shapes}</g>"#)
			}
		}
		DigitStyle::SevenSegment => body_segments(text, settings),
	};
	let svg = format!(
		concat!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144">"#,
			r#"<rect width="144" height="144" fill="{bg}"/>"#,
			"{body}</svg>"
		),
		bg = bg,
		body = body,
	);
	format!("data:image/svg+xml;base64,{}", BASE64.encode(svg))
}

/// 文字样式：用宿主字体渲染，字号随字符数自适应
fn body_text(text: &str, settings: &DigitSettings) -> String {
	let fg = safe_color(&settings.color, "#ffffff");
	let (font_size, baseline_y) = match text.trim().chars().count() {
		0 | 1 => (112, 112),
		2 => (80, 101),
		count => {
			let size = 168 / count;
			(size, 72 + size * 3 / 8)
		}
	};
	format!(
		r#"<text x="72" y="{y}" font-family="sans-serif" font-size="{size}" font-weight="bold" fill="{fg}" text-anchor="middle">{text}</text>"#,
		y = baseline_y,
		size = font_size,
		fg = fg,
		text = text,
	)
}

/// 七段样式：逐字符自绘矢量段，不依赖任何字体，五个平台画出来一模一样
fn body_segments(text: &str, settings: &DigitSettings) -> String {
	let chars: Vec<char> = text.chars().collect();
	if chars.is_empty() {
		return String::new();
	}
	let l = layout(chars.len(), settings.slant);

	let mut out = String::new();
	for (i, c) in chars.iter().copied().enumerate() {
		let shapes = match c {
			':' => colon_dots(),
			'-' | '0'..='9' => glyph_shapes(c, settings),
			// 空白位（12 小时制的空时十位、熄灭的冒号）只留背景
			_ => continue,
		};
		let x = l.left + l.advance * i as f64;
		let transform = if settings.slant {
			format!("translate({},{}) skewX({}) scale({})", num(x), num(l.top), num(-SLANT_DEG), num(l.scale))
		} else {
			format!("translate({},{}) scale({})", num(x), num(l.top), num(l.scale))
		};
		out.push_str(&format!(r#"<g transform="{transform}">{shapes}</g>"#));
	}
	if out.is_empty() {
		return String::new();
	}
	let fg = safe_color(&settings.color, "#ffffff");
	format!(r#"<g fill="{fg}">{out}</g>"#)
}

/// 一行七段字符的排版结果：画布内居中，并已补偿倾斜的左缘突出量
#[derive(Debug)]
struct Layout {
	scale: f64,
	advance: f64,
	#[cfg(test)]
	row_w: f64,
	#[cfg(test)]
	skew: f64,
	left: f64,
	top: f64,
}

fn layout(chars: usize, slant: bool) -> Layout {
	let (height, spacing) =
		if chars > 1 { (PAIR_HEIGHT, PAIR_SPACING) } else { (GLYPH_HEIGHT, 0.0) };
	let skew_ratio = if slant { SLANT_DEG.to_radians().tan() } else { 0.0 };
	let chars = chars as f64;
	// 长数字串按比例缩小字身和字距，并为两侧各留 8 像素。
	let width = BOX_W * height / BOX_H * chars + spacing * (chars - 1.0) + height * skew_ratio;
	let fit = ((TILE - 16.0) / width).min(1.0);
	let (height, spacing) = (height * fit, spacing * fit);
	let scale = height / BOX_H;
	let advance = BOX_W * scale + spacing;
	let row_w = advance * chars - spacing;
	let skew = skew_ratio * height;
	Layout {
		scale,
		advance,
		#[cfg(test)]
		row_w,
		#[cfg(test)]
		skew,
		// 倾斜使字身下缘向左突出，整体右移半个突出量才能保持居中
		left: (TILE - row_w + skew) / 2.0,
		top: (TILE - height) / 2.0,
	}
}

/// 单个数字或短横线的多边形（暗段画在点亮段之前）
fn glyph_shapes(c: char, settings: &DigitSettings) -> String {
	let lit = if c == '-' {
		DASH_SEGMENT
	} else {
		SEGMENTS[(c as u8 - b'0') as usize]
	};
	let show_ghost = c != '-' && settings.ghost_segments;
	let mut ghost = String::new();
	let mut on = String::new();
	for (i, pts) in SEG_POLYGONS.iter().enumerate() {
		let shape = format!(r##"<polygon points="{pts}"/>"##);
		if lit & (1 << i) != 0 {
			on.push_str(&shape);
		} else if show_ghost {
			ghost.push_str(&shape);
		}
	}
	if ghost.is_empty() {
		on
	} else {
		format!(r#"<g fill-opacity="{}">{ghost}</g>{on}"#, num(GHOST_OPACITY))
	}
}

/// 冒号：以归一化字身中心为基准，上下各画一颗切角方点
fn colon_dots() -> String {
	const HALF: i32 = 64;
	const CORNER: i32 = 32;
	const RISE: i32 = 180;
	let cx = BOX_W as i32 / 2;
	let cy = BOX_H as i32 / 2;
	let dot = |y: i32| {
		let points = polygon_points(&[
			(cx - CORNER, y - HALF), (cx + CORNER, y - HALF),
			(cx + HALF, y - CORNER), (cx + HALF, y + CORNER),
			(cx + CORNER, y + HALF), (cx - CORNER, y + HALF),
			(cx - HALF, y + CORNER), (cx - HALF, y - CORNER),
		]);
		format!(r#"<polygon points="{points}"/>"#)
	};
	format!("{}{}", dot(cy - RISE), dot(cy + RISE))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn settings(style: DigitStyle) -> DigitSettings {
		DigitSettings {
			style,
			position: Position::HourOnes,
			..DigitSettings::default()
		}
	}

	/// 把 render_svg 产出的 data URI 解回 SVG 明文，便于断言
	fn svg_of(uri: &str) -> String {
		let payload = uri.strip_prefix("data:image/svg+xml;base64,").expect("data uri prefix");
		String::from_utf8(BASE64.decode(payload).expect("valid base64")).expect("utf8 svg")
	}

	/// 渲染七段样式并返回 SVG 明文
	fn segments(text: &str, settings: &DigitSettings) -> String {
		svg_of(&render_svg(text, settings))
	}

	/// 搬移回归：文字样式产出的 data URI 必须与重构前逐字节相同
	#[test]
	fn text_style_output_is_unchanged() {
		let expected = |body: &str| -> String {
			format!(
				"data:image/svg+xml;base64,{}",
				BASE64.encode(format!(
					r##"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144"><rect width="144" height="144" fill="#000000"/>{body}</svg>"##
				))
			)
		};
		assert_eq!(
			render_svg("9", &settings(DigitStyle::Text)),
			expected(r##"<text x="72" y="112" font-family="sans-serif" font-size="112" font-weight="bold" fill="#ffffff" text-anchor="middle">9</text>"##)
		);
		assert_eq!(
			render_svg("09", &settings(DigitStyle::Text)),
			expected(r##"<text x="72" y="101" font-family="sans-serif" font-size="80" font-weight="bold" fill="#ffffff" text-anchor="middle">09</text>"##)
		);
	}

	#[test]
	fn segment_table_matches_seven_segment_encoding() {
		// 标准共阴编码：0=0x3F 1=0x06 … 8=0x7F 9=0x6F
		let expected = [0x3Fu8, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
		assert_eq!(SEGMENTS, expected);
	}

	/// 暗段组（<g fill-opacity="…">…</g>）里的 polygon 数
	fn ghost_polygons(body: &str) -> usize {
		match body.split_once(r##"fill-opacity=""##) {
			Some((_, rest)) => {
				rest.split_once("</g>").expect("ghost group must close").0.matches("<polygon").count()
			}
			None => 0,
		}
	}

	#[test]
	fn lit_segments_per_digit() {
		let s = settings(DigitStyle::SevenSegment);
		for digit in '0'..='9' {
			let body = segments(&digit.to_string(), &s);
			let lit = SEGMENTS[(digit as u8 - b'0') as usize].count_ones();
			// 点亮段 + 暗段 = 完整字身 7 段，且各画一次
			assert_eq!(body.matches("<polygon").count(), 7, "digit {digit}");
			assert_eq!(ghost_polygons(&body) as u32, 7 - lit, "digit {digit} lit {lit}");
		}
	}

	#[test]
	fn ghost_segments_toggle() {
		let mut s = settings(DigitStyle::SevenSegment);
		assert!(segments("1", &s).contains(r##"fill-opacity="0.08""##));
		s.ghost_segments = false;
		let without = segments("1", &s);
		assert!(!without.contains("fill-opacity"), "{without}");
		// 关掉暗段后只剩点亮的那 2 段
		assert_eq!(without.matches("<polygon").count(), 2);
	}

	#[test]
	fn slant_toggle_and_centering_compensation() {
		let mut s = settings(DigitStyle::SevenSegment);
		assert!(segments("8", &s).contains("skewX(-8)"));
		s.slant = false;
		assert!(!segments("8", &s).contains("skewX"));
	}

	#[test]
	fn two_digits_are_side_by_side_and_one_digit_centered() {
		let s = settings(DigitStyle::SevenSegment);
		let one = segments("8", &s);
		let two = segments("12", &s);
		assert_eq!(one.matches(r#"<g transform="translate"#).count(), 1);
		assert_eq!(two.matches(r#"<g transform="translate"#).count(), 2);
		// 两位同格时字身缩小，避免与倾斜突出量一起顶出画布
		assert!(one.contains(&format!("scale({})", num(layout(1, true).scale))), "{one}");
		assert!(two.contains(&format!("scale({})", num(layout(2, true).scale))), "{two}");
		assert!(layout(2, true).scale < layout(1, true).scale);
	}

	#[test]
	fn full_year_renders_in_both_styles() {
		let text = svg_of(&render_svg("2026", &settings(DigitStyle::Text)));
		assert!(text.contains(r#"font-size="42""#));
		assert!(text.contains(">2026</text>"));

		let year = segments("2026", &settings(DigitStyle::SevenSegment));
		assert_eq!(year.matches(r#"<g transform="translate"#).count(), 4);
	}

	#[test]
	fn seven_segment_layout_stays_inside_the_tile() {
		for chars in [1, 2, 4] {
			for slant in [true, false] {
				let l = layout(chars, slant);
				let left_edge = l.left - l.skew;
				let right_edge = l.left + l.row_w;
				assert!(left_edge >= 0.0, "clipped left: {l:?}");
				assert!(right_edge <= TILE, "clipped right: {l:?}");
				assert!(l.top >= 0.0 && l.top + l.scale * BOX_H <= TILE, "clipped vertically: {l:?}");
				// 真正的居中：左右留白等量
				assert!((left_edge - (TILE - right_edge)).abs() < 0.01, "off-center: {l:?}");
			}
		}
	}

	#[test]
	fn colon_is_two_dots_and_blank_is_empty() {
		let mut s = settings(DigitStyle::SevenSegment);
		for slant in [true, false] {
			s.slant = slant;
			let colon = segments(":", &s);
			let dash = segments("-", &s);
			assert_eq!(colon.matches("<polygon").count(), 2);
			assert_eq!(colon.contains("skewX(-8)"), slant);
			assert_eq!(colon.split("<polygon").next(), dash.split("<polygon").next());
			assert!(!colon.contains("fill-opacity"));
		}
		// 空格（熄灭冒号 / 12 小时制空时十位）只剩背景，什么都不画
		assert!(!segments(" ", &s).contains("<g"));
		assert!(!segments(" ", &s).contains("<polygon"));
	}

	#[test]
	fn dash_renders_in_both_styles() {
		let text = svg_of(&render_svg("-", &settings(DigitStyle::Text)));
		assert!(text.contains(">-</text>"));

		let mut s = settings(DigitStyle::SevenSegment);
		for slant in [true, false] {
			s.slant = slant;
			let dash = segments("-", &s);
			let middle = &SEG_POLYGONS[6];
			assert!(dash.contains(&format!(r#"<polygon points="{middle}"/>"#)));
			assert_eq!(dash.matches("<polygon").count(), 1);
			assert_eq!(dash.contains("skewX(-8)"), slant);
			assert!(!dash.contains("fill-opacity"));
		}
	}

	#[test]
	fn unlit_separators_keep_their_shape_when_ghost_segments_are_enabled() {
		for (position, separator) in [(Position::Colon, ":"), (Position::Dash, "-")] {
			let mut s = settings(DigitStyle::SevenSegment);
			s.position = position;
			let lit = body_segments(separator, &s);
			let dim = segments(" ", &s);
			assert!(dim.contains(&format!(r#"<g fill-opacity="0.08">{lit}</g>"#)));

			s.ghost_segments = false;
			assert!(!segments(" ", &s).contains("<polygon"));
		}
	}

	#[test]
	fn seven_segment_output_does_not_embed_text() {
		// 七段样式不把字符拼进 SVG，注入面为零
		let s = settings(DigitStyle::SevenSegment);
		let body = segments("8", &s);
		assert!(!body.contains(">8<"));
	}

	#[test]
	fn bad_colors_fall_back() {
		let mut s = settings(DigitStyle::Text);
		s.color = "javascript:alert(1)".to_owned();
		s.background = "#fff\"></path>".to_owned();
		let svg = svg_of(&render_svg("9", &s));
		assert!(svg.contains(r##"fill="#000000""##), "{svg}");
		assert!(svg.contains(r##"fill="#ffffff""##), "{svg}");
	}

	/// 解析某个段的顶点串
	fn points(seg: usize) -> Vec<(i32, i32)> {
		SEG_POLYGONS[seg]
			.split_whitespace()
			.map(|p| {
				let (x, y) = p.split_once(',').expect("x,y pair");
				(x.parse().expect("int x"), y.parse().expect("int y"))
			})
			.collect()
	}

	/// 数码管的味道来自段间那条斜缝：横竖段的尖角必须各自退缩，不能插进对方中线把角糊死
	#[test]
	fn segment_tips_meet_in_an_open_seam() {
		let half = SEG_T / 2;
		assert!(SEG_GAP > 0, "段间必须留缝");
		// a 顶横段：左右尖角落在两条竖段中线之间，而不是越过
		let a = points(0);
		assert_eq!(a[0], (half + SEG_GAP, half));
		assert_eq!(a[3], (BOX_W as i32 - half - SEG_GAP, half));
		// f 左上竖段：上尖角退到 a 中线下方
		assert_eq!(points(5)[0], (half, half + SEG_GAP));
		// 尖角是 45° 斜切：从尖角到平边横竖各走半个段宽
		assert_eq!((a[1].0 - a[0].0, a[0].1 - a[1].1), (half, half));
	}

	/// 字身必须正好占满归一化框：居中与两位同格的字距都按 BOX_W/BOX_H 推算
	#[test]
	fn glyph_fills_the_normalized_box() {
		let all: Vec<(i32, i32)> = (0..7).flat_map(points).collect();
		let xs: Vec<i32> = all.iter().map(|p| p.0).collect();
		let ys: Vec<i32> = all.iter().map(|p| p.1).collect();
		assert_eq!((xs.iter().min(), xs.iter().max()), (Some(&0), Some(&(BOX_W as i32))));
		assert_eq!((ys.iter().min(), ys.iter().max()), (Some(&0), Some(&(BOX_H as i32))));
	}
}
