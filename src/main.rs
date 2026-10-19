use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use eframe::egui;
use image::DynamicImage;
use std::path::PathBuf;
#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex};

struct App {
	current_file: Option<PathBuf>,
	image: Option<DynamicImage>,
	texture: Option<egui::TextureHandle>,

	patient_name: String,
	pixel_spacing: f64,

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

#[cfg(not(target_arch = "wasm32"))]
fn open_tiff(ctx: &egui::Context) -> Option<(PathBuf, DynamicImage, egui::TextureHandle)> {
	let path = rfd::FileDialog::new().add_filter("TIFF", &["tif", "tiff"]).pick_file()?;

	let img = image::open(&path).ok()?;

	let preview = img.to_rgba8();

	let size = [preview.width() as usize, preview.height() as usize];

	let color_image = egui::ColorImage::from_rgba_unmultiplied(size, preview.as_raw());

	let texture = ctx.load_texture("preview", color_image, Default::default());

	Some((path, img, texture))
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
					let preview = img.to_rgba8();

					let size = [preview.width() as usize, preview.height() as usize];

					let color_image =
						egui::ColorImage::from_rgba_unmultiplied(size, preview.as_raw());

					let texture = ui.ctx().load_texture("preview", color_image, Default::default());

					self.current_file = Some(PathBuf::from(filename));

					self.image = Some(img);
					self.texture = Some(texture);
				}
			}
		}

		egui::Frame::default().fill(egui::Color32::WHITE).show(ui, |ui| {
			ui.style_mut().visuals = egui::Visuals::light();

			ui.heading("TIFF to DICONDE Converter");

			if ui.button("Open TIFF File").clicked() {
				#[cfg(not(target_arch = "wasm32"))]
				{
					if let Some((path, img, tex)) = open_tiff(ui.ctx()) {
						self.current_file = Some(path);
						self.image = Some(img);
						self.texture = Some(tex);
					}
				}

				#[cfg(target_arch = "wasm32")]
				{
					start_open_tiff(self.pending_image.clone());
				}
			}

			if let Some(path) = &self.current_file {
				ui.label(format!("Loaded: {}", path.display()));
			}

			ui.separator();

			ui.label("Component Name (0010,0010)");

			egui::Frame::group(ui.style()).show(ui, |ui| {
				ui.add_sized([350.0, 24.0], egui::TextEdit::singleline(&mut self.patient_name));
			});

			ui.separator();

			ui.horizontal(|ui| {
				ui.label("Pixel Spacing (0028,0030)");

				ui.add_sized(
					[100.0, 24.0],
					egui::DragValue::new(&mut self.pixel_spacing).speed(0.01).fixed_decimals(6),
				);
			});

			ui.separator();

			#[cfg(not(target_arch = "wasm32"))]
			if ui.button("Save As DICONDE").clicked() {
				if let Some(img) = &self.image {
					save_dicom(img, &self.patient_name, self.pixel_spacing);
				}
			}

			#[cfg(target_arch = "wasm32")]
			if ui.button("Download DICONDE").clicked() {
				if let Some(img) = &self.image {
					save_dicom(img, &self.patient_name, self.pixel_spacing);
				}
			}

			ui.separator();

			if let Some(texture) = &self.texture {
				let size = texture.size_vec2();

				let scale = (ui.available_width() / size.x).min(ui.available_height() / size.y);

				ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale));
			}
		});
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
		"TIFF to DICOM",
		options,
		Box::new(|cc| {
			cc.egui_ctx.set_visuals(egui::Visuals::light());
			cc.egui_ctx.set_theme(egui::Theme::Light);
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
					cc.egui_ctx.set_theme(egui::Theme::Light);
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
