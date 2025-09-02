use eframe::egui;
use egui::{Label, RichText};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use egui_plotter::EguiBackend;
use plotters::prelude::*;

pub struct ScopeData {
    pub samples: Vec<(f32,f32)>,
    pub rms: f32,
    pub peak: f32,
}

impl ScopeData {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            rms: 0.0,
            peak: 0.0,
        }
    }
}

#[derive(Default)]
pub struct ScopeCtl {
    pub data: Vec<Mutex<ScopeData>>,
    cur: AtomicU8,
}

impl ScopeCtl {
    pub fn new() -> Self {
        let mut data: Vec<Mutex<ScopeData>> = Vec::new();
        data.push(Mutex::new(ScopeData::new()));
        data.push(Mutex::new(ScopeData::new()));
        Self {
            data: data,
            cur: AtomicU8::new(0)
        }
    }

    pub fn cur(&self) -> usize {
        self.cur.load(Ordering::Relaxed) as usize
    }
}

pub fn run_scope(ctl: Arc<ScopeCtl>) {
    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native("Scope", native_options, Box::new(|cc| Ok(Box::new(ScopeBuilder::new(cc, ctl)))));
}

#[derive(Default)]
struct ScopeBuilder {
    ctl: Arc<ScopeCtl>
}

impl ScopeBuilder {
    fn new(cc: &eframe::CreationContext<'_>, ctl: Arc<ScopeCtl>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self { ctl }
    }
}

impl eframe::App for ScopeBuilder {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let data = self.ctl.data[self.ctl.cur()].lock().unwrap();
            let rms = data.rms;
            let peak = data.peak;
            ui.vertical(|ui| {
                if let Some(last_sample) = data.samples.last() {
                    let frame = egui::Frame::new()
                        .corner_radius(20.0);
                    frame.show(ui, |ui| {
                        ui.set_width(400.0);
                        ui.set_height(300.0);

                        let root = EguiBackend::new(ui).into_drawing_area();
                        root.fill(&BLACK).unwrap();
                        let mut chart = ChartBuilder::on(&root)
                            .margin(5)
                            .x_label_area_size(30)
                            .y_label_area_size(30)
                            .build_cartesian_2d(0f32..last_sample.0, -1f32..1f32)
                            .unwrap();

                        chart.configure_mesh()
                            .axis_style(WHITE)
                            .label_style(("sans-serif", 10).into_font().color(&WHITE))
                            .draw().unwrap();

                        chart
                            .draw_series(LineSeries::new(data.samples.clone(), &GREEN))
                            .unwrap();

                        // chart
                        //     .configure_series_labels()
                        //     .background_style(WHITE.mix(0.8))
                        //     .border_style(BLACK)
                        //     .draw()
                        //     .unwrap();

                        root.present().unwrap();
                    });
                }
                ui.add(Label::new(RichText::new(format!("rms {rms} peak {peak}")).monospace()));
            });

        });
       ctx.request_repaint_after_secs(1.0/20.0);
   }
}

