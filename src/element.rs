use std::{collections::HashMap, num::NonZeroUsize, sync::Arc};

use vello::{
    kurbo::{Affine, Circle, CubicBez, Point, Rect, RoundedRect, Size, Vec2},
    peniko::{Brush, Color, Fill, Stroke},
    SceneBuilder, SceneFragment,
};
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    window::CursorIcon,
};

use crate::{ComponentHandle, RegisteredPin, Simulator};

pub struct EventContext<'a> {
    pub set_cursor_icon: &'a dyn Fn(CursorIcon),
}

pub struct ElementManager {
    sim: Arc<Simulator>,
    pin_cache: HashMap<RegisteredPin, Point>,

    view: Rect,

    zoom: f64,
    translation: Vec2,
    mouse_position: Vec2,
    last_mouse_position: Vec2,
    drag: bool,

    grabbed_element: Option<NonZeroUsize>,
    pub elements: Vec<Element>,
}

impl ElementManager {
    pub fn new(sim: Arc<Simulator>, size: (f64, f64)) -> ElementManager {
        ElementManager {
            sim,
            pin_cache: HashMap::new(),
            view: Rect::from_origin_size((0.0, 0.0), size),

            zoom: 1.0,
            translation: Vec2::ZERO,
            mouse_position: Vec2::ZERO,
            last_mouse_position: Vec2::ZERO,
            drag: false,

            grabbed_element: None,
            elements: Vec::new(),
        }
    }

    pub fn with_elements(mut self, elements: Vec<Element>) -> ElementManager {
        self.elements = elements;
        self.elements
            .iter_mut()
            .for_each(|e| e.calculate_positions(&self.sim, &mut self.pin_cache));
        self
    }

    pub fn insert(&mut self, mut element: Element) {
        element.calculate_positions(&self.sim, &mut self.pin_cache);
        self.elements.push(element);
    }

    pub fn event(&mut self, ctx: &EventContext, window_event: &WindowEvent) {
        match window_event {
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Middle,
                ..
            } => {
                self.drag = *state == ElementState::Pressed;

                (ctx.set_cursor_icon)(match state {
                    ElementState::Pressed => CursorIcon::Move,
                    ElementState::Released => CursorIcon::Default,
                })
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let pos = self.mouse_position / self.zoom - self.translation / self.zoom;

                for (i, element) in self.elements.iter().enumerate() {
                    let result = element.hittest(&self.sim, &self.pin_cache, pos.to_point());

                    match result {
                        HitResult::Hit => {
                            debug_assert!(self.grabbed_element.is_none());
                            self.grabbed_element = Some((i + 1).try_into().unwrap());
                        }
                        _ => (),
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let _ = self.grabbed_element.take();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Vec2::new(position.x, position.y);

                if self.drag {
                    self.translation += self.mouse_position - self.last_mouse_position;
                } else if let Some(element) = self.grabbed_element {
                    let index = element.get() - 1;

                    self.elements[index].position +=
                        (self.mouse_position - self.last_mouse_position) / self.zoom;
                    self.elements[index].calculate_positions(&self.sim, &mut self.pin_cache);
                }

                self.last_mouse_position = self.mouse_position;
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(_, y),
                phase: TouchPhase::Moved,
                ..
            } => {
                let mut factor = 1.0 / 0.8;
                if (*y as f64) < 0.0 {
                    factor = 1.0 / factor;
                }

                self.zoom *= factor;
                let dx = (self.mouse_position.x - self.translation.x) * (factor - 1.0);
                let dy = (self.mouse_position.y - self.translation.y) * (factor - 1.0);
                self.translation -= Vec2::new(dx, dy);
            }
            _ => (),
        }
    }

    pub fn draw(&mut self, builder: &mut SceneBuilder, _bounds: &Rect, _mode: i8) {
        let mut elements_fragment = SceneFragment::new();
        let mut elements_builder = SceneBuilder::for_fragment(&mut elements_fragment);

        for element in &self.elements {
            element.draw(&mut elements_builder, &mut self.pin_cache, &self.sim);
        }

        builder.append(
            &elements_fragment,
            if _mode == 0 {
                Some(Affine::scale(self.zoom).then_translate(self.translation))
            } else {
                None
            },
        );

        let mut connection_fragment = SceneFragment::new();
        let mut connection_builder = SceneBuilder::for_fragment(&mut connection_fragment);

        for (p1, p2) in self.sim.in_to_out.iter() {
            let Some(p1) = self.pin_cache.get(p1) else {
                eprintln!("Point2 doesn't exist!");
                continue;
            };

            let Some(p2) = self.pin_cache.get(p2) else {
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
                if ctrl1.x > ctrl2.x {
                    (Point::new(ctrl2.x, ctrl1.y), Point::new(ctrl1.x, ctrl2.y))
                } else {
                    (ctrl1, ctrl2)
                }
            } else {
                if ctrl1.x < ctrl2.x {
                    (Point::new(ctrl1.x, ctrl2.y), Point::new(ctrl2.x, ctrl1.y))
                } else {
                    (ctrl2, ctrl1)
                }
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
            Some(Affine::scale(self.zoom).then_translate(self.translation)),
        );
    }
}

#[derive(Debug)]
pub enum HitResult {
    Hit,
    HitInput,
    HitOutput,
    NoHit,
}

pub struct Element {
    pub component: ComponentHandle,

    pub input_size: usize,
    pub output_size: usize,

    pub position: Point,
    pub size: Size,
}

impl Element {
    pub fn new(component: ComponentHandle, position: Point) -> Element {
        Element {
            component,
            position,
            size: Size::new(100.0, 100.0),
            input_size: 0,
            output_size: 0,
        }
    }

    pub fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }

    pub fn hittest(
        &self,
        sim: &Simulator,
        pin_cache: &HashMap<RegisteredPin, Point>,
        point: Point,
    ) -> HitResult {
        let inputs = self.input_size;
        {
            for i in 1..inputs + 1 {
                let Some(pin) = sim.out_to_in.get(&self.component.to_pin(i - 1)) else {
                    continue;
                };

                let input_pos = *pin_cache.get(pin).unwrap();

                if Rect::from_center_size(input_pos, (10.0, 10.0)).contains(point) {
                    return HitResult::HitInput;
                }
            }
        }

        {
            let outputs = self.output_size;

            for i in 1..outputs + 1 {
                let pin = self.component.to_pin(i - 1 + inputs); // simulator i/o pins share the same indicies so add input length as offset for outputs
                let Some(pin) = sim.in_to_out.get(&pin) else {
                    continue;
                };

                let output_pos = *pin_cache.get(pin).unwrap();

                if Rect::from_center_size(output_pos, (10.0, 10.0)).contains(point) {
                    return HitResult::HitOutput;
                }
            }
        }

        if Rect::from_origin_size(self.position, self.size).contains(point) {
            return HitResult::Hit;
        }

        HitResult::NoHit
    }

    pub fn calculate_positions(
        &mut self,
        sim: &Simulator,
        pin_cache: &mut HashMap<RegisteredPin, Point>,
    ) {
        let component = sim.get_component(&self.component);
        let component = component.borrow();
        let inputs = component.input_len();
        {
            let calc_input = |i| {
                let input_offset = self.size.height / (inputs + 1) as f64;
                Point {
                    x: self.position.x,
                    y: self.position.y + input_offset * i as f64,
                }
            };

            for i in 1..inputs + 1 {
                if let Some(pin) = sim.out_to_in.get(&self.component.to_pin(i - 1)) {
                    pin_cache.insert(*pin, calc_input(i));
                }
            }
        }

        self.input_size = inputs;
        self.output_size = component.output_len();
        {
            let outputs = self.output_size;

            let calc_output = |i| {
                let output_offset = self.size.height / (outputs + 1) as f64;
                Point {
                    x: self.position.x + self.size.width,
                    y: self.position.y + output_offset * i as f64,
                }
            };

            for i in 1..outputs + 1 {
                let pin = self.component.to_pin(i - 1 + inputs); // simulator i/o pins share the same indicies so add input length as offset for outputs
                if let Some(pin) = sim.in_to_out.get(&pin) {
                    pin_cache.insert(*pin, calc_output(i));
                }
            }
        }
    }

    pub fn draw(
        &self,
        builder: &mut SceneBuilder,
        pin_cache: &mut HashMap<RegisteredPin, Point>,
        sim: &Simulator,
    ) {
        // Draw the body
        builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(Color::rgb(0.2, 0.2, 0.2)),
            None,
            &RoundedRect::from_origin_size(self.position, (self.size.width, self.size.height), 5.0),
        );

        let inputs = self.input_size;

        {
            let calc_input = |i| {
                let input_offset = self.size.height / (inputs + 1) as f64;
                Point {
                    x: self.position.x,
                    y: self.position.y + input_offset * i as f64,
                }
            };

            for i in 1..inputs + 1 {
                if let Some(pin) = sim.out_to_in.get(&self.component.to_pin(i - 1)) {
                    let input_pos = *pin_cache.entry(*pin).or_insert_with(|| calc_input(i));

                    // Pin is connected
                    builder.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &Brush::Solid(Color::GREEN),
                        None,
                        &Circle::new(input_pos, 5.0),
                    );
                } else {
                    let input_pos = calc_input(i);
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
            let outputs = self.output_size;

            let calc_output = |i| {
                let output_offset = self.size.height / (outputs + 1) as f64;
                Point {
                    x: self.position.x + self.size.width,
                    y: self.position.y + output_offset * i as f64,
                }
            };

            for i in 1..outputs + 1 {
                let pin = self.component.to_pin(i - 1 + inputs); // simulator i/o pins share the same indicies so add input length as offset for outputs
                if let Some(pin) = sim.in_to_out.get(&pin) {
                    let output_pos = *pin_cache.entry(*pin).or_insert_with(|| calc_output(i));

                    // Pin is connected
                    builder.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &Brush::Solid(Color::GREEN),
                        None,
                        &Circle::new(output_pos, 5.0),
                    );
                } else {
                    let output_pos = calc_output(i);
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
