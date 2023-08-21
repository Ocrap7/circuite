use std::{collections::HashMap, sync::Arc};

use vello::{
    kurbo::{Affine, BezPath, Circle, CubicBez, Line, Point, Rect, RoundedRect, Vec2},
    peniko::{BlendMode, Brush, Color, Fill, Mix, Stroke, Style},
    SceneBuilder, SceneFragment,
};
use winit::event::{MouseScrollDelta, TouchPhase, WindowEvent};

use crate::{ComponentHandle, RegisteredPin, Simulator};

pub struct ElementManager {
    sim: Arc<Simulator>,

    zoom: f64,
    translation: Vec2,
    view_translation: Vec2,
    // translation: Affine,
    mouse_position: Vec2,
    // view: Rect,
    pub elements: Vec<Element>,
}

impl ElementManager {
    pub fn new(sim: Arc<Simulator>) -> ElementManager {
        ElementManager {
            sim,
            zoom: 1.0,
            translation: Vec2::ZERO,
            view_translation: Vec2::ZERO,
            mouse_position: Vec2::ZERO,
            // view: Rect::ZERO,
            elements: Vec::new(),
        }
    }

    pub fn with_elements(mut self, elements: Vec<Element>) -> ElementManager {
        self.elements = elements;
        self
    }

    pub fn insert(&mut self, element: Element) {
        self.elements.push(element);
    }

    pub fn event(&mut self, window_event: &WindowEvent) {
        match window_event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Vec2::new(position.x, position.y);
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(x, y),
                phase: TouchPhase::Moved,
                ..
            } => {
                // let trans = Affine::translate(-self.translation)
                //     .then_scale(self.zoom)
                //     .then_translate(self.translation)
                //     .inverse();
                // let multiplier = 0.012;
                // let y = *y as f64;
                // let scaling = if y > 0.0 {
                //     ((-y).log10() * multiplier).exp()
                // } else {
                //     (y.log10() * multiplier).exp()
                // };

                let view_matrix = Affine::translate(-self.mouse_position)
                    .then_scale(self.zoom)
                    .then_translate(self.mouse_position);

                let rect = Rect::from_origin_size(self.mouse_position.to_point(), (0.0, 0.0));
                let world_mouse = view_matrix.inverse().transform_rect_bbox(rect).origin();

                // let scaling = scaling.clamp(0.01, 100.0);
                self.zoom += *y as f64 * 0.1 * self.zoom;

                //  let = (view.xmax - view.xmin) * (scaling-1.)

                // self.translation -= self.mouse_position;
                // self.translation *= if *y > 0.0 {
                //     1.05
                // } else {
                //     1.0 / 1.05
                // };

                // let point = Point::new(self.mouse_position.x, self.mouse_position.y);

                // let tpoint = trans
                //     .transform_rect_bbox(Rect::from_origin_size(point, (0.0, 0.0)))
                //     .origin();

                println!("{:?} {:?}", self.translation, world_mouse);
                self.translation = world_mouse.to_vec2();

                let view_matrix = Affine::translate(-self.translation)
                    .then_scale(self.zoom)
                    .then_translate(self.mouse_position);

                let rect = Rect::from_origin_size(self.mouse_position.to_point(), (0.0, 0.0));
                let world_mouse = view_matrix.inverse().transform_rect_bbox(rect).origin();

                self.view_translation = self.mouse_position;

                // self.translation = Affine::translate(self.mouse_position);
            }
            _ => (),
        }
    }

    pub fn draw(&self, builder: &mut SceneBuilder, bounds: &Rect) {
        let mut pin_cache = HashMap::new();

        // println!("{:?}", bounds.size());
        // builder.push_layer(
        //     Mix::Normal,
        //     1.0,
        //     Affine::translate((0.0, 0.0)),
        //     &bounds,
        // );
        let view_matrix = Affine::translate(-self.translation)
            .then_scale(self.zoom)
            .then_translate(self.translation);

        let rect = Rect::from_origin_size(self.mouse_position.to_point(), (0.0, 0.0));
        let world_mouse = view_matrix.inverse().transform_rect_bbox(rect).origin();

        // println!("{:?}", world_mouse);

        let mut elements_fragment = SceneFragment::new();
        let mut elements_builder = SceneBuilder::for_fragment(&mut elements_fragment);

        for element in &self.elements {
            element.draw(&mut elements_builder, &mut pin_cache, &self.sim);
        }

        elements_builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::RED,
            None,
            &Rect::from_origin_size((bounds.width() / 2.0, bounds.height() / 2.0), (10.0, 10.0)),
        );

        builder.append(
            &elements_fragment,
            Some(
                // Affine::translate(Vec2::new(50.0, 50.0))
                //     .pre_scale(2.0)
                //     .then_translate(-Vec2::new(50.0, 50.0)),
                // Affine::translate(-Vec2::new(bounds.width() / 2.0, bounds.height() / 2.0))
                //     .then_scale(self.zoom)
                //     .then_translate((50.0, 50.0).into()), // .then_translate(Vec2::new(bounds.width() / 2.0, bounds.height() / 2.0)),
                // self.translation.inverse().then_scale(self.zoom) * self.translation,
                Affine::translate(-self.translation)
                    .then_scale(self.zoom)
                    .then_translate(self.view_translation),
            ),
            // Affine::translate(p),
        );

        builder.stroke(
            &Stroke::new(1.0),
            Affine::IDENTITY,
            &Color::RED,
            None,
            &Line::new((50.5, 0.0), (50.5, 500.0)),
        );

        builder.stroke(
            &Stroke::new(1.0),
            Affine::IDENTITY,
            &Color::RED,
            None,
            &Line::new((0.0, 50.5), (500.0, 50.5)),
        );

        // builder.push_layer(
        //     Mix::Normal,
        //     1.0,
        //     Affine::translate(self.translation).then_scale(self.zoom),
        //     &bounds,
        // );
        let mut connection_fragment = SceneFragment::new();
        let mut connection_builder = SceneBuilder::for_fragment(&mut connection_fragment);

        for (p1, p2) in self.sim.in_to_out.iter() {
            let Some(p1) = pin_cache.get(p1) else  {
                eprintln!("Point2 doesn't exist!");
                continue;
            };

            let Some(p2) = pin_cache.get(p2) else  {
                eprintln!("Point2 doesn't exist!");
                continue;
            };

            let rect = Rect::from_points(*p1, *p2);
            let cx = 60.0;
            let cy = 0.0;

            let col = Brush::Solid(Color::GREEN);
            let ctrl1 = Point::new(rect.max_x() - cx, rect.max_y() - cy);
            let ctrl2 = Point::new(rect.min_x() + cx, rect.min_y() + cy);

            let (ctrl1, ctrl2) = if p1.y >= p2.y {
                if ctrl1.x < ctrl2.x {
                    (Point::new(ctrl2.x, ctrl1.y), Point::new(ctrl1.x, ctrl2.y))
                } else {
                    (ctrl1, ctrl2)
                }
            } else {
                (ctrl2, ctrl1)
            };

            connection_builder.stroke(
                &Stroke::new(2.0),
                Affine::IDENTITY,
                &col,
                None,
                &CubicBez::new(*p1, ctrl1, ctrl2, *p2),
            );
        }

        builder.append(
            &connection_fragment,
            Some(
                // Affine::translate(-self.translation)
                Affine::translate(-self.translation)
                    .then_scale(self.zoom)
                    .then_translate(self.translation),
            ),
        );
    }
}

pub struct Element {
    pub component: ComponentHandle,

    pub position: Point,
}

impl Element {
    pub fn draw(
        &self,
        builder: &mut SceneBuilder,
        pin_cache: &mut HashMap<RegisteredPin, Point>,
        sim: &Arc<Simulator>,
    ) {
        let width = 100.0;
        let height = 100.0;

        // Draw the body
        builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(Color::rgb(0.2, 0.2, 0.2)),
            None,
            &RoundedRect::from_origin_size(self.position, (width, height), 5.0),
        );

        let component = sim.get_component(&self.component);
        let component = component.borrow();
        let inputs = component.input_len();

        {
            let mut input_pos = self.position.clone();

            let input_offset = height / (inputs + 1) as f64;

            for i in 1..inputs + 1 {
                input_pos.y = self.position.y + input_offset * i as f64;

                if let Some(pin) = sim.out_to_in.get(&self.component.to_pin(i - 1)) {
                    pin_cache.insert(*pin, input_pos);

                    // Pin is connected
                    builder.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &Brush::Solid(Color::GREEN),
                        None,
                        &Circle::new(input_pos, 5.0),
                    );
                } else {
                    // Pin is not connected
                    builder.stroke(
                        &Stroke::new(2.0),
                        Affine::IDENTITY,
                        &Brush::Solid(Color::GREEN),
                        None,
                        &Circle::new(input_pos, 5.0),
                    );
                }
            }
        }

        {
            let outputs = component.output_len();
            let mut output_pos = self.position.clone();
            output_pos.x += width;

            let output_offset = height / (outputs + 1) as f64;

            for i in 1..outputs + 1 {
                output_pos.y = self.position.y + output_offset * i as f64;

                let pin = self.component.to_pin(i - 1 + inputs); // simulator i/o pins share the same indicies so add input length as offset for outputs
                if let Some(pin) = sim.in_to_out.get(&pin) {
                    pin_cache.insert(*pin, output_pos);

                    // Pin is connected
                    builder.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &Brush::Solid(Color::GREEN),
                        None,
                        &Circle::new(output_pos, 5.0),
                    );
                } else {
                    // Pin is not connected
                    builder.stroke(
                        &Stroke::new(2.0),
                        Affine::IDENTITY,
                        &Brush::Solid(Color::GREEN),
                        None,
                        &Circle::new(output_pos, 5.0),
                    );
                }
            }
        }
    }
}
