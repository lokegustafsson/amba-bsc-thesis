use std::fmt;

use egui::{self, Color32 as Colour32, Rect, Response, Sense, Stroke, Ui, Widget};
use emath::Vec2;

use crate::{EmbeddingParameters, Graph2D, LodText};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ColouringMode {
	AllGrey,
	ByState,
	StronglyConnectedComponents,
	Function,
}

impl fmt::Display for ColouringMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ColouringMode::AllGrey => write!(f, "All grey"),
			ColouringMode::ByState => write!(f, "By state"),
			ColouringMode::StronglyConnectedComponents => {
				write!(f, "Strongly connected components")
			}
			ColouringMode::Function => write!(f, "Function"),
		}
	}
}

impl Widget for &mut EmbeddingParameters {
	fn ui(self, ui: &mut Ui) -> Response {
		const STEPS: f64 = 10.0;
		ui.vertical(|ui| {
			let resp = ui
				.add(
					egui::Slider::new(
						&mut self.noise,
						0.0..=EmbeddingParameters::MAX_NOISE,
					)
					.step_by(EmbeddingParameters::MAX_NOISE / STEPS)
					.text("noise"),
				)
				.union(
					ui.add(
						egui::Slider::new(
							&mut self.attraction,
							(EmbeddingParameters::MAX_ATTRACTION / STEPS)
								..=EmbeddingParameters::MAX_ATTRACTION,
						)
						.step_by(EmbeddingParameters::MAX_ATTRACTION / STEPS)
						.text("attraction"),
					),
				)
				.union(
					ui.add(
						egui::Slider::new(
							&mut self.repulsion,
							(EmbeddingParameters::MAX_REPULSION / STEPS)
								..=EmbeddingParameters::MAX_REPULSION,
						)
						.step_by(EmbeddingParameters::MAX_REPULSION / STEPS)
						.text("repulsion"),
					),
				)
				.union(
					ui.add(
						egui::Slider::new(
							&mut self.gravity,
							0.0..=EmbeddingParameters::MAX_GRAVITY,
						)
						.step_by(EmbeddingParameters::MAX_GRAVITY / STEPS)
						.text("gravity"),
					),
				);
			if self.statistic_updates_per_second == 0.0 {
				ui.label("Updates per second: paused");
			} else if self.statistic_updates_per_second.is_finite() {
				ui.label(format!(
					"Updates per second: {:.0}00",
					self.statistic_updates_per_second / 100.0
				));
			}
			resp
		})
		.inner
	}
}

pub struct GraphWidget {
	/// Linear zoom level:
	/// 1x => the graph fits within the area with some margin
	/// 10x => we are looking at a small part of the graph
	zoom: f32,
	pos: Vec2,
	active_node_and_pan: Option<(usize, PanState)>,
	pub new_priority_node: Option<usize>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PanState {
	Centering,
	Centered,
	Off,
}
impl Default for GraphWidget {
	fn default() -> Self {
		Self {
			zoom: 1.0,
			pos: Vec2::ZERO,
			active_node_and_pan: None,
			new_priority_node: None,
		}
	}
}

impl GraphWidget {
	pub fn reset_view(&mut self) {
		*self = Self::default();
	}

	pub fn active_node_id(&self) -> Option<usize> {
		self.active_node_and_pan.map(|(node, _)| node)
	}

	pub fn show(&mut self, ui: &mut Ui, graph: &Graph2D, colouring_mode: ColouringMode) {
		egui::Frame::none()
			.stroke(ui.visuals().widgets.inactive.fg_stroke)
			.show(ui, |ui| {
				let height = ui.available_height();
				let width = ui.available_width();
				ui.set_clip_rect({
					let mut clip = ui.cursor();
					clip.set_height(height);
					clip.set_width(width);
					clip
				});

				let scroll_area = egui::ScrollArea::both()
					.auto_shrink([false, false])
					.scroll_offset(self.pos)
					.show_viewport(ui, |ui, viewport| {
						draw_graph(
							ui,
							self.zoom,
							&mut self.active_node_and_pan,
							&mut self.new_priority_node,
							viewport,
							graph,
							colouring_mode,
						)
					});

				let (zoom_delta, latest_pointer_pos) = ui.input(|input| {
					(
						input.zoom_delta() + input.scroll_delta.y * 2.0 / height,
						input.pointer.interact_pos(),
					)
				});
				let background_drag = scroll_area.inner.drag_delta();

				let real_zoom_delta = if let (true, Some(hover_pos)) =
					(ui.ui_contains_pointer(), latest_pointer_pos)
				{
					// Scroll to zoom
					let new_zoom = (self.zoom * zoom_delta).max(1.0);
					let real_zoom_delta = new_zoom / self.zoom;
					self.zoom = new_zoom;

					let hover_pos = if self
						.active_node_and_pan
						.map_or(false, |(_, pan)| pan != PanState::Off)
					{
						scroll_area.inner_rect.size() / 2.0
					} else {
						hover_pos - scroll_area.inner_rect.min
					};
					let new_offset = (self.pos + hover_pos) * real_zoom_delta - hover_pos;
					self.pos = new_offset;
					real_zoom_delta
				} else {
					1.0
				};
				if background_drag != emath::Vec2::ZERO {
					// Drag to pan
					self.pos -= background_drag;
					if let Some((_, pan)) = self.active_node_and_pan.as_mut() {
						*pan = PanState::Off;
					}
				} else {
					// Automatic pan to active node
					if let Some((active, pan @ (PanState::Centering | PanState::Centered))) =
						&mut self.active_node_and_pan
					{
						let active_pos = translate_embed_to_scrollarea_pos(
							graph.node_positions[*active],
							graph,
							self.zoom,
							scroll_area.inner_rect.size(),
						)
						.to_vec2() - scroll_area.inner_rect.size() / 2.0;

						match pan {
							PanState::Centering => {
								const TAKE_FOCUS_SPEED: f32 = 0.5;
								self.pos = self.pos * (1.0 - TAKE_FOCUS_SPEED)
									+ active_pos * TAKE_FOCUS_SPEED;
								if (self.pos - active_pos).length() < self.pos.length() * 1e-6 {
									*pan = PanState::Centered;
								}
							}
							PanState::Centered => self.pos = active_pos,
							PanState::Off => unreachable!(),
						}
					}
				}

				//  Clip scrollarea position to content size
				self.pos = self.pos.max(emath::Vec2::ZERO).min(
					scroll_area.content_size * real_zoom_delta - scroll_area.inner_rect.size(),
				);
			});
	}
}

fn draw_graph(
	ui: &mut Ui,
	zoom_level: f32,
	active_node_and_pan: &mut Option<(usize, PanState)>,
	priority_node: &mut Option<usize>,
	viewport: Rect,
	graph: &Graph2D,
	colouring_mode: ColouringMode,
) -> egui::Response {
	let offset = ui.cursor().left_top().to_vec2();
	let background = ui.allocate_rect(
		Rect::from_min_size(offset.to_pos2(), ui.available_size() * zoom_level),
		egui::Sense::drag(),
	);
	let style_widgets = &ui.visuals().widgets.clone();
	let style_selection = &ui.visuals().selection.clone();

	let scrollarea_node_pos: Vec<emath::Pos2> = graph
		.node_positions
		.iter()
		.map(|&p| translate_embed_to_scrollarea_pos(p, graph, zoom_level, viewport.size()))
		.collect();

	let node_size = {
		let mut node_size = vec![std::f32::INFINITY; graph.node_positions.len()];
		for &(a, b) in &graph.edges {
			if a != b {
				let d = scrollarea_node_pos[a].distance(scrollarea_node_pos[b]);
				node_size[a] = node_size[a].min(0.7 * d);
				node_size[b] = node_size[b].min(0.7 * d);
			}
		}
		node_size
	};

	let expanded_viewport = viewport.expand(
		node_size
			.iter()
			.copied()
			.reduce(|a, b| a.max(b))
			.unwrap_or(0.0)
			/ 2.0,
	);

	for (i, &pos) in scrollarea_node_pos.iter().enumerate() {
		if expanded_viewport.contains(pos) {
			let default_colour = match colouring_mode {
				ColouringMode::AllGrey => style_widgets.hovered.bg_fill,

				ColouringMode::ByState => get_colour(graph.node_drawing_data[i].state),
				ColouringMode::StronglyConnectedComponents => {
					get_colour(graph.node_drawing_data[i].scc_group)
				}
				ColouringMode::Function => get_colour(graph.node_drawing_data[i].function),
			};
			let (bg_colour, stroke) = if active_node_and_pan.map_or(false, |(node, _)| node == i) {
				(style_selection.bg_fill, style_selection.stroke)
			} else {
				(default_colour, style_widgets.hovered.bg_stroke)
			};
			let node = draw_node(
				ui,
				bg_colour,
				stroke,
				pos,
				node_size[i],
				&graph.node_drawing_data[i].lod_text,
				offset,
			);
			if node.double_clicked() {
				*priority_node = Some(i);
			}
			if node.drag_started() {
				*active_node_and_pan = Some((i, PanState::Centering));
			}
		}
	}

	for &(a, b) in &graph.edges {
		let origin = scrollarea_node_pos[a];
		let target = scrollarea_node_pos[b];
		if !viewport.intersects(Rect::from_two_pos(origin, target)) {
			continue;
		}
		edge_arrow(
			ui.painter(),
			origin + offset,
			target - origin,
			node_size[a],
			node_size[b],
			style_widgets.hovered.fg_stroke,
		);
	}
	background
}

fn draw_node(
	ui: &mut Ui,
	bg_colour: Colour32,
	stroke: Stroke,
	pos: egui::Pos2,
	node_width: f32,
	text: &LodText,
	offset: Vec2,
) -> Response {
	let rect =
		Rect::from_center_size(pos, egui::Vec2::new(node_width, node_width)).translate(offset);
	let resp = ui.allocate_rect(rect, Sense::click_and_drag());
	let rounding = node_width / 5.0;
	let (lod_text, lod_size, lod_center) = {
		let useful_node_width = 0.6 * node_width;
		const SHAPE: f32 = 2.0;
		let (lod_text, w, h) = {
			let font_height = ui.text_style_height(&egui::style::TextStyle::Small);
			let height = (useful_node_width / font_height) as u32;
			let width = (SHAPE * useful_node_width / font_height) as u32;
			text.get_given_available_square(width, height)
		};
		let num_lines = lod_text.lines().count();
		let lod_size = {
			let width = w as f32 / SHAPE;
			let height = h as f32;
			(useful_node_width / f32::max(width, height)).clamp(1.0, 400.0)
		};
		let lod_center = num_lines <= 2;
		(lod_text, lod_size, lod_center)
	};

	ui.put(rect, move |ui: &mut Ui| {
		if lod_text.is_empty() {
			ui.painter().rect_filled(rect, rounding, bg_colour);
			ui.painter().rect_stroke(rect, rounding, stroke);
		} else {
			egui::Frame::none()
				.rounding(rounding)
				.fill(bg_colour)
				.stroke(stroke)
				.inner_margin(egui::style::Margin::same(node_width * 0.1))
				.show(ui, |ui| {
					ui.with_layout(
						egui::Layout::top_down(if lod_center {
							egui::Align::Center
						} else {
							egui::Align::Min
						}),
						|ui| {
							ui.label(egui::RichText::new(lod_text).monospace().size(lod_size));
						},
					);
					ui.allocate_space(ui.available_size());
				});
		}
		resp
	})
}

fn edge_arrow(
	painter: &egui::Painter,
	mut origin: egui::Pos2,
	vec: egui::Vec2,
	node_size_origin: f32,
	node_size_target: f32,
	stroke: egui::Stroke,
) {
	let mut tip = origin + vec;
	let margin_origin = node_size_origin / 2.0 * vec / vec.abs().max_elem();
	let margin_tip = node_size_target / 2.0 * vec / vec.abs().max_elem();
	origin += margin_origin;
	tip -= margin_tip;

	let rot = emath::Rot2::from_angle(std::f32::consts::TAU / 10.0);
	let tip_length = ((tip - origin).length() / 4.0).min(node_size_target / 3.0);
	let dir = vec.normalized();
	painter.line_segment([origin, tip], stroke);
	painter.line_segment([tip, tip - tip_length * (rot * dir)], stroke);
	painter.line_segment(
		[tip, tip - tip_length * (rot.inverse() * dir)],
		stroke,
	);
}

fn translate_embed_to_scrollarea_pos(
	embed_space_pos: glam::DVec2,
	graph: &Graph2D,
	zoom_level: f32,
	viewport_size: Vec2,
) -> emath::Pos2 {
	let unit_square_pos =
		glam_to_emath((embed_space_pos - graph.min) / (graph.max - graph.min).max_element());
	let nozoom_viewport_margin = (viewport_size.min_elem() / 20.0).min(50.0);
	let nozoom_content_width = (viewport_size.min_elem() - 2.0 * nozoom_viewport_margin) * 0.9;
	let nozoom_offset = (viewport_size - emath::Vec2::splat(nozoom_content_width)) / 2.0;
	let nozoom_pos = nozoom_offset + unit_square_pos * nozoom_content_width;
	(nozoom_pos * zoom_level).to_pos2()
}

fn glam_to_emath(v: glam::DVec2) -> emath::Vec2 {
	emath::Vec2::from(<[f32; 2]>::from(v.as_vec2()))
}

fn get_colour(i: usize) -> Colour32 {
	const COLOURS: [Colour32; 10] = [
		Colour32::from_rgb(0xA8, 0x58, 0x4D),
		Colour32::from_rgb(0xE1, 0xF5, 0xA2),
		Colour32::from_rgb(0xF5, 0x95, 0x89),
		Colour32::from_rgb(0x71, 0xA6, 0xF5),
		Colour32::from_rgb(0x56, 0x77, 0xA8),
		Colour32::from_rgb(0xA8, 0x97, 0x4D),
		Colour32::from_rgb(0x63, 0xF5, 0xEC),
		Colour32::from_rgb(0xF5, 0xE1, 0x89),
		Colour32::from_rgb(0xF5, 0x71, 0xE1),
		Colour32::from_rgb(0xA8, 0x56, 0x9C),
	];

	COLOURS[i % COLOURS.len()]
}
