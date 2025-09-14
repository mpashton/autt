use eframe::egui;
use egui::{Label, RichText};
use std::sync::{Arc, Mutex};
use egui_plotter::EguiBackend;
use plotters::prelude::*;
use egui_taffy::{taffy, tui, TuiBuilderLogic, TuiBuilder, TuiWidget};
//use taffy;

#[derive(Clone)]
pub struct ScopeChannel {
    pub name: String,
    pub samples: Vec<(f32,f32)>,
    pub fft: Vec<(f32, f32)>,
    pub rms: f32,
    pub peak: f32,
}

impl ScopeChannel {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            samples: Vec::new(),
            fft: Vec::new(),
            rms: 0.0,
            peak: 0.0,
        }
    }
}

#[derive(Default)]
pub struct Scope {
    pub data: Mutex<Vec<ScopeChannel>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new())
        }
    }
}

pub fn run_scope(ctl: Arc<Scope>) {
    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native("Scope", native_options, Box::new(|cc| Ok(Box::new(ScopeBuilder::new(cc, ctl)))));
}

#[derive(Default)]
struct ScopeBuilder {
    ctl: Arc<Scope>
}

impl ScopeBuilder {
    fn new(cc: &eframe::CreationContext<'_>, ctl: Arc<Scope>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self { ctl }
    }
}

impl TuiWidget for ScopeChannel {
    type Response = egui::Response;

    fn taffy_ui(self, tuib: TuiBuilder) -> Self::Response {
        tuib.ui_add_manual(|ui| {
            let rms = self.rms;
            let peak = self.peak;
            ui.vertical(|ui| {
                if let Some(last_sample) = self.samples.last() {
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
                            .draw_series(LineSeries::new(self.samples.clone(), &GREEN))
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
            }).response
        },
        |mut response, _ui| response)
    }
}

fn scope_channel(channel: &ScopeChannel) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| {
        let rms = channel.rms;
        let peak = channel.peak;
        ui.vertical(|ui| {
            if let Some(last_sample) = channel.samples.last() {
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
                        .draw_series(LineSeries::new(channel.samples.clone(), &GREEN))
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
        }).response
    }
}

impl eframe::App for ScopeBuilder {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let channel_ct = self.ctl.data.lock().unwrap().len();
        egui::CentralPanel::default().show(ctx, |ui| {
            tui(ui, ui.id().with("demo"))
                .reserve_available_space()
                .style(taffy::Style {
                    flex_wrap: taffy::FlexWrap::Wrap,
                    ..Default::default()
                })
                .show(|tui| {
                    for i in 0..channel_ct {
                        let data = self.ctl.data.lock().unwrap();
                        tui.ui_add(data[i].clone());
                    }
                });
        });
        ctx.request_repaint_after_secs(1.0/20.0);
    }
}

