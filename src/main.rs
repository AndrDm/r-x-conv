use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use eframe::egui;
use image::DynamicImage;
use std::path::PathBuf;
#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex};

const OPEN_SHORTCUT: egui::KeyboardShortcut =
	egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);

const SAVE_SHORTCUT: egui::KeyboardShortcut =
	egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);

#[cfg(not(target_arch = "wasm32"))]
const QUIT_SHORTCUT: egui::KeyboardShortcut =
	egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Q);

const OPEN_ICON: egui::ImageSource = egui::include_image!("../assets/icons/open.svg");

const SAVE_ICON: egui::ImageSource = egui::include_image!("../assets/icons/save.svg");

#[derive(Clone, Copy)]
enum MenuCommand {
	Open,
	Save,
	#[cfg(not(target_arch = "wasm32"))]
	Quit,
}

fn menu_item(
	ui: &mut egui::Ui,
	label: &str,
	shortcut: egui::KeyboardShortcut,
	enabled: bool,
) -> egui::Response {
	ui.add_enabled(
		enabled,
		egui::Button::new(label).shortcut_text(ui.ctx().format_shortcut(&shortcut)),
	)
}

// White, frameless fill for panels/CentralPanel (the app uses a fixed light look;
// egui's default panel frames would follow the context theme instead).
fn light_panel_frame() -> egui::Frame {
	egui::Frame::new()
		.fill(egui::Color32::WHITE)
		.inner_margin(egui::Margin::symmetric(8, 2))
		.stroke(egui::Stroke::NONE)
}

fn toolbar_button(
	ui: &mut egui::Ui,
	icon: egui::ImageSource,
	tooltip: &str,
	enabled: bool,
) -> egui::Response {
	let response = ui.add_enabled(
		enabled,
		egui::Button::image(egui::Image::new(icon).fit_to_exact_size(egui::Vec2::splat(20.0))),
	);

	response.on_hover_text(tooltip)
}

impl App {
	// Stores a newly loaded image and builds the preview texture.
	fn set_image(&mut self, path: Option<PathBuf>, image: DynamicImage, ctx: &egui::Context) {
		let texture = make_preview(ctx, &image);

		self.current_file = path;

		self.image = Some(image);

		self.texture = Some(texture);

		// Default the component name to the image file name (without extension),
		// but only while the field is still blank, so an edited name survives loading.
		if self.patient_name.is_empty()
			&& let Some(stem) =
				self.current_file.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str())
		{
			self.patient_name = stem.to_owned();
		}
	}

	// Shows the toolbar (menu bar + icon buttons) and handles global keyboard shortcuts.
	// Returns None if no command was triggered this frame.
	fn show_menu(&mut self, ui: &mut egui::Ui) -> Option<MenuCommand> {
		let mut command = None;

		if ui.input_mut(|i| i.consume_shortcut(&OPEN_SHORTCUT)) {
			command = Some(MenuCommand::Open);
		}

		if ui.input_mut(|i| i.consume_shortcut(&SAVE_SHORTCUT)) {
			command = Some(MenuCommand::Save);
		}

		#[cfg(not(target_arch = "wasm32"))]
		if ui.input_mut(|i| i.consume_shortcut(&QUIT_SHORTCUT)) {
			command = Some(MenuCommand::Quit);
		}

		let save_enabled = self.image.is_some();

		let mut open_about = false;

		egui::MenuBar::new().ui(ui, |ui| {
			ui.menu_button("File", |ui| {
				if menu_item(ui, "Open TIFF Image...", OPEN_SHORTCUT, true).clicked() {
					command = Some(MenuCommand::Open);
				}

				ui.separator();

				#[cfg(target_arch = "wasm32")]
				if menu_item(ui, "Download...", SAVE_SHORTCUT, save_enabled).clicked() {
					command = Some(MenuCommand::Save);
				}

				#[cfg(not(target_arch = "wasm32"))]
				if menu_item(ui, "Save DICONDE Image As...", SAVE_SHORTCUT, save_enabled).clicked()
				{
					command = Some(MenuCommand::Save);
				}

				ui.separator();

				#[cfg(not(target_arch = "wasm32"))]
				if menu_item(ui, "Quit", QUIT_SHORTCUT, true).clicked() {
					command = Some(MenuCommand::Quit);
				}
			});

			ui.menu_button("Help", |ui| {
				if ui.button("About...").clicked() {
					open_about = true;
				}
			});

			// Image open/save toolbar buttons.
			ui.separator();

			#[cfg(not(target_arch = "wasm32"))]
			let save_tooltip = "Save DICONDE Image As...";

			#[cfg(target_arch = "wasm32")]
			let save_tooltip = "Download DICONDE";

			if toolbar_button(ui, OPEN_ICON, "Open TIFF File", true).clicked() {
				command = Some(MenuCommand::Open);
			}

			if toolbar_button(ui, SAVE_ICON, save_tooltip, save_enabled).clicked() {
				command = Some(MenuCommand::Save);
			}
		});

		if open_about {
			self.show_about = true;
		}

		command
	}
}

struct App {
	current_file: Option<PathBuf>,
	image: Option<DynamicImage>,
	texture: Option<egui::TextureHandle>,

	patient_name: String,
	pixel_spacing: f64,

	show_about: bool,

	#[cfg(target_arch = "wasm32")]
	pending_image: Arc<Mutex<Option<(String, Vec<u8>)>>>,
}

impl Default for App {
	fn default() -> Self {
		Self {
			current_file: None,
			image: None,
			texture: None,
			patient_name: String::new(),
			pixel_spacing: 0.1,
			show_about: false,

			#[cfg(target_arch = "wasm32")]
			pending_image: Arc::new(Mutex::new(None)),
		}
	}
}

#[cfg(target_arch = "wasm32")]
fn start_open_tiff(pending: std::sync::Arc<std::sync::Mutex<Option<(String, Vec<u8>)>>>) {
	use rfd::AsyncFileDialog;

	wasm_bindgen_futures::spawn_local(async move {
		if let Some(file) =
			AsyncFileDialog::new().add_filter("TIFF", &["tif", "tiff"]).pick_file().await
		{
			let name = file.file_name();

			let data = file.read().await;

			*pending.lock().unwrap() = Some((name, data));
		}
	});
}

// Builds the egui preview texture for an image.
fn make_preview(ctx: &egui::Context, img: &DynamicImage) -> egui::TextureHandle {
	let preview = img.to_rgba8();

	let size = [preview.width() as usize, preview.height() as usize];

	let color_image = egui::ColorImage::from_rgba_unmultiplied(size, preview.as_raw());

	ctx.load_texture("preview", color_image, Default::default())
}

// True if the path ends with a .tif or .tiff extension (case-insensitive).
fn is_tiff(path: &std::path::Path) -> bool {
	let extension = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());

	matches!(extension.as_deref(), Some("tif" | "tiff"))
}

// Loads the first TIFF among the dropped files (native version, blocking read).
#[cfg(not(target_arch = "wasm32"))]
fn load_dropped_image(files: &[egui::DroppedFileHandle]) -> Option<(PathBuf, DynamicImage)> {
	let file = files.iter().find(|file| is_tiff(file.path()))?;

	let path = file.path().to_path_buf();

	image::open(&path).ok().map(|img| (path, img))
}

// Starts an async read of the first TIFF among the dropped files (web version);
// the bytes are queued into `pending` and picked up by the UI on a later frame.
#[cfg(target_arch = "wasm32")]
fn start_load_dropped(
	pending: std::sync::Arc<std::sync::Mutex<Option<(String, Vec<u8>)>>>,
	files: Vec<egui::DroppedFileHandle>,
) {
	wasm_bindgen_futures::spawn_local(async move {
		let Some(file) = files.iter().find(|file| is_tiff(file.path())).cloned() else {
			return;
		};

		let name = file.path().to_string_lossy().into_owned();

		match file.bytes_async().await {
			Ok(bytes) => *pending.lock().unwrap() = Some((name, bytes)),
			Err(e) => web_sys::console::error_1(&format!("Dropped file read error: {e}").into()),
		}
	});
}

#[cfg(not(target_arch = "wasm32"))]
fn open_tiff() -> Option<(PathBuf, DynamicImage)> {
	let path = rfd::FileDialog::new().add_filter("TIFF", &["tif", "tiff"]).pick_file()?;

	image::open(&path).ok().map(|img| (path, img))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_dicom(image: &DynamicImage, patient_name: &str, pixel_spacing: f64) {
	let path = match rfd::FileDialog::new()
		.add_filter("DICOM/DICONDE", &["dcm"])
		.set_file_name("image.dcm")
		.save_file()
	{
		Some(p) => p,
		None => return,
	};

	let gray = image.to_luma16();

	let mut obj = InMemDicomObject::new_empty();

	obj.put(DataElement::new(Tag(0x0010, 0x0010), VR::PN, PrimitiveValue::from(patient_name)));

	obj.put(DataElement::new(
		Tag(0x0028, 0x0010),
		VR::US,
		PrimitiveValue::from(gray.height() as u16),
	));

	obj.put(DataElement::new(
		Tag(0x0028, 0x0011),
		VR::US,
		PrimitiveValue::from(gray.width() as u16),
	));

	obj.put(DataElement::new(
		Tag(0x0028, 0x0030),
		VR::DS,
		PrimitiveValue::from(format!("{}\\{}", pixel_spacing, pixel_spacing)),
	));

	obj.put(DataElement::new(Tag(0x0028, 0x0002), VR::US, PrimitiveValue::from(1u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0004), VR::CS, PrimitiveValue::from("MONOCHROME2")));

	obj.put(DataElement::new(Tag(0x0028, 0x0100), VR::US, PrimitiveValue::from(16u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0101), VR::US, PrimitiveValue::from(16u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0102), VR::US, PrimitiveValue::from(15u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0103), VR::US, PrimitiveValue::from(0u16)));

	let pixel_data: Vec<u16> = gray.pixels().map(|p| p.0[0]).collect();

	obj.put(DataElement::new(Tag(0x7FE0, 0x0010), VR::OW, PrimitiveValue::U16(pixel_data.into())));

	let file_obj =
		match obj.with_meta(FileMetaTableBuilder::new().transfer_syntax("1.2.840.10008.1.2.1")) {
			Ok(v) => v,
			Err(e) => {
				eprintln!("Failed to create DICONDE metadata: {e}");
				return;
			}
		};

	if let Err(e) = file_obj.write_to_file(&path) {
		eprintln!("Failed to save DICONDE: {e}");
	} else {
		println!("Saved {}", path.display());
	}
}

#[cfg(target_arch = "wasm32")]
fn save_dicom(image: &DynamicImage, patient_name: &str, pixel_spacing: f64) {
	use std::io::Cursor;

	use wasm_bindgen::JsCast;
	use web_sys::{Blob, Url};

	let gray = image.to_luma16();

	let mut obj = InMemDicomObject::new_empty();

	obj.put(DataElement::new(Tag(0x0010, 0x0010), VR::PN, PrimitiveValue::from(patient_name)));

	obj.put(DataElement::new(
		Tag(0x0028, 0x0010),
		VR::US,
		PrimitiveValue::from(gray.height() as u16),
	));

	obj.put(DataElement::new(
		Tag(0x0028, 0x0011),
		VR::US,
		PrimitiveValue::from(gray.width() as u16),
	));

	obj.put(DataElement::new(
		Tag(0x0028, 0x0030),
		VR::DS,
		PrimitiveValue::from(format!("{}\\{}", pixel_spacing, pixel_spacing)),
	));

	obj.put(DataElement::new(Tag(0x0028, 0x0002), VR::US, PrimitiveValue::from(1u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0004), VR::CS, PrimitiveValue::from("MONOCHROME2")));

	obj.put(DataElement::new(Tag(0x0028, 0x0100), VR::US, PrimitiveValue::from(16u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0101), VR::US, PrimitiveValue::from(16u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0102), VR::US, PrimitiveValue::from(15u16)));

	obj.put(DataElement::new(Tag(0x0028, 0x0103), VR::US, PrimitiveValue::from(0u16)));

	let pixel_data: Vec<u16> = gray.pixels().map(|p| p.0[0]).collect();

	obj.put(DataElement::new(Tag(0x7FE0, 0x0010), VR::OW, PrimitiveValue::U16(pixel_data.into())));

	let file_obj =
		match obj.with_meta(FileMetaTableBuilder::new().transfer_syntax("1.2.840.10008.1.2.1")) {
			Ok(v) => v,
			Err(e) => {
				web_sys::console::error_1(&format!("DICONDE meta error: {e}").into());
				return;
			}
		};

	let mut cursor = Cursor::new(Vec::<u8>::new());

	if let Err(e) = file_obj.write_all(&mut cursor) {
		web_sys::console::error_1(&format!("Write error: {e}").into());
		return;
	}

	let bytes = cursor.into_inner();

	let array = js_sys::Uint8Array::from(bytes.as_slice());

	let seq = js_sys::Array::new();

	seq.push(&array.buffer());

	let blob = Blob::new_with_u8_array_sequence(&seq).unwrap();

	let url = Url::create_object_url_with_blob(&blob).unwrap();

	let document = web_sys::window().unwrap().document().unwrap();

	let a = document.create_element("a").unwrap();

	a.set_attribute("href", &url).unwrap();

	a.set_attribute("download", "image.dcm").unwrap();

	let a: web_sys::HtmlElement = a.dyn_into().unwrap();

	a.click();

	Url::revoke_object_url(&url).unwrap();
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		#[cfg(target_arch = "wasm32")]
		{
			let pending = {
				let mut guard = self.pending_image.lock().unwrap();
				guard.take()
			};

			if let Some((filename, bytes)) = pending {
				if let Ok(img) = image::load_from_memory(&bytes) {
					self.set_image(Some(PathBuf::from(filename)), img, ui.ctx());
				}
			}
		}

		let dropped_files = ui.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));

		if !dropped_files.is_empty() {
			#[cfg(not(target_arch = "wasm32"))]
			if let Some((path, img)) = load_dropped_image(&dropped_files) {
				self.set_image(Some(path), img, ui.ctx());
			}

			#[cfg(target_arch = "wasm32")]
			start_load_dropped(self.pending_image.clone(), dropped_files);
		}

		// App title (web only — on desktop the OS window title serves this purpose):
		#[cfg(target_arch = "wasm32")]
		egui::Panel::top(egui::Id::new("title_bar")).frame(light_panel_frame()).show(ui, |ui| {
			ui.label(egui::RichText::new("TIFF to DICONDE Converter").strong().size(18.0));
		});

		// Toolbar (menu + image open/save buttons):
		let toolbar = egui::Panel::top(egui::Id::new("toolbar"))
			.frame(light_panel_frame())
			.show(ui, |ui| self.show_menu(ui));

		let menu_command = toolbar.inner;

		// Status bar:
		egui::Panel::bottom(egui::Id::new("status_bar")).frame(light_panel_frame()).show(
			ui,
			|ui| {
				ui.horizontal(|ui| match &self.current_file {
					Some(path) => {
						ui.label(format!("Loaded: {}", path.display()));
					}

					None => {
						ui.weak("No image loaded");
					}
				});
			},
		);

		// Main content:
		egui::CentralPanel::default().frame(light_panel_frame()).show(ui, |ui| {
			ui.style_mut().visuals = egui::Visuals::light();

			ui.label("Component Name (0010,0010)");

			egui::Frame::group(ui.style()).show(ui, |ui| {
				ui.add_sized([350.0, 24.0], egui::TextEdit::singleline(&mut self.patient_name));
			});

			ui.add_space(8.0);

			ui.horizontal(|ui| {
				let tooltip = "Physical distance in the component between the center of each \
                   pixel, specified internally by a numeric pair: adjacent row \
                   spacing and adjacent column spacing, in mm.";

				ui.label("Pixel Spacing, mm/pix (0028,0030)")
					.on_hover_text(tooltip);

				ui.add_sized(
					[100.0, 24.0],
					egui::DragValue::new(&mut self.pixel_spacing)
						.speed(0.01)
						.fixed_decimals(6)
						.range(0.0..=f64::INFINITY),
				)
					.on_hover_text(tooltip);
			});

			ui.add_space(8.0);

			if let Some(texture) = &self.texture {
				let size = texture.size_vec2();

				let scale = (ui.available_width() / size.x).min(ui.available_height() / size.y);

				ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale));
			} else {
				ui.with_layout(
					egui::Layout::centered_and_justified(egui::Direction::TopDown),
					|ui| {
						ui.weak("Drag & drop a TIFF file here, or use File > Open TIFF Image...");
					},
				);
			}
		});

		if let Some(command) = menu_command {
			match command {
				MenuCommand::Open => {
					#[cfg(not(target_arch = "wasm32"))]
					if let Some((path, img)) = open_tiff() {
						self.set_image(Some(path), img, ui.ctx());
					}

					#[cfg(target_arch = "wasm32")]
					start_open_tiff(self.pending_image.clone());
				}

				MenuCommand::Save => {
					if let Some(img) = &self.image {
						save_dicom(img, &self.patient_name, self.pixel_spacing);
					}
				}

				#[cfg(not(target_arch = "wasm32"))]
				MenuCommand::Quit => {
					ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
				}
			}
		}

		if self.show_about {
			let response = egui::Modal::new(egui::Id::new("about")).show(ui.ctx(), |ui| {
				ui.set_width(280.0);

				ui.vertical_centered(|ui| {
					ui.heading(env!("CARGO_PKG_NAME"));

					ui.weak(concat!("v", env!("CARGO_PKG_VERSION")));
				});

				ui.add_space(8.0);

				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					ui.button("OK").clicked()
				})
				.inner
			});

			if response.inner || response.should_close() {
				self.show_about = false;
			}
		}
	}
}

// desktop version
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
		..Default::default()
	};

	eframe::run_native(
		"TIFF to DICONDE Converter",
		options,
		Box::new(|cc| {
			// Use a light theme regardless of the OS theme (set on both styles, since the
			// theme preference decides which one is active at runtime).
			cc.egui_ctx.set_theme(egui::Theme::Light);
			cc.egui_ctx.set_visuals_of(egui::Theme::Dark, egui::Visuals::light());
			cc.egui_ctx.set_visuals_of(egui::Theme::Light, egui::Visuals::light());
			egui_extras::install_image_loaders(&cc.egui_ctx);
			Ok(Box::new(App::default()))
		}),
	)
}

// web version
#[cfg(target_arch = "wasm32")]
fn main() {
	use eframe::wasm_bindgen::JsCast as _;

	let web_options = eframe::WebOptions::default();

	wasm_bindgen_futures::spawn_local(async {
		let document = web_sys::window().expect("No window").document().expect("No document");

		let canvas = document
			.get_element_by_id("the_canvas_id")
			.expect("Failed to find the_canvas_id")
			.dyn_into::<web_sys::HtmlCanvasElement>()
			.expect("the_canvas_id was not a HtmlCanvasElement");

		let start_result = eframe::WebRunner::new()
			.start(
				canvas,
				web_options,
				Box::new(|cc| {
					// Use a light theme regardless of the OS theme (set on both styles, since
					// the theme preference decides which one is active at runtime).
					cc.egui_ctx.set_theme(egui::Theme::Light);
					cc.egui_ctx.set_visuals_of(egui::Theme::Dark, egui::Visuals::light());
					cc.egui_ctx.set_visuals_of(egui::Theme::Light, egui::Visuals::light());
					egui_extras::install_image_loaders(&cc.egui_ctx);
					Ok(Box::new(App::default())) //MyApp
				}),
			)
			.await;

		let loading_text = web_sys::window()
			.and_then(|w| w.document())
			.and_then(|d| d.get_element_by_id("loading_text"));

		if let Some(loading_text) = loading_text {
			match start_result {
				Ok(_) => loading_text.remove(),
				Err(e) => {
					loading_text.set_inner_html("<p> The app crashed (see console). </p>");
					panic!("Failed to start eframe: {e:?}");
				}
			}
		}
	});
}
