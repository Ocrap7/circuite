use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{Debug, Display},
    rc::Rc,
    sync::{Arc, Mutex},
};

mod element;
mod messages;
mod platform;
mod render;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Low,
    High,
    Invalid,
}

use cacao::appkit::{toolbar::Toolbar, window::WindowToolbarStyle};
use element::{Element, ElementManager};
use messages::Message;
use objc::{msg_send, runtime::Object, sel, sel_impl};
use objc_id::ShareId;
use platform::toolbars::PreferencesToolbar;
use render::RenderManager;
use vello::kurbo::{Rect, Affine};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::macos::{WindowBuilderExtMacOS, WindowExtMacOS},
    window::WindowBuilder, dpi::LogicalSize,
};
use State::*;

#[tokio::main]
async fn main() {
    let event_loop = EventLoopBuilder::<Message>::with_user_event().build();

    // cacao::appkit::App::new(bundle_id, delegate).

    let window = WindowBuilder::new()
        .with_title("Node Fiddler")
        .with_inner_size(LogicalSize::new(800, 772))
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    // window.ns_window();

    let mut sim = Arc::new(Simulator::new());
    let mut elements = Vec::new();

    let tb = Toolbar::new("PreferencesToolbar", PreferencesToolbar::default());
    tb.set_display_mode(cacao::appkit::toolbar::ToolbarDisplayMode::IconAndLabel);

    let win = unsafe {
        cacao::appkit::window::Window::<()> {
            objc: ShareId::from_ptr(window.ns_window() as *mut Object),
            delegate: None,
        }
    };
    win.set_title_visibility(cacao::appkit::window::TitleVisibility::Hidden);
    win.set_toolbar(&tb);
    // win.styl
    unsafe {
        // sel_impl!()
        let _: () = msg_send![&*(window.ns_window() as *mut Object), setToolbarStyle:WindowToolbarStyle::Unified];
    }

    let trans = Affine::translate((-50.0, -50.0)).then_scale(2.0).then_translate((50.0, 50.0).into());
    println!("{:?}", trans.transform_rect_bbox(Rect::from_origin_size((0.0, 0.0), (100.0, 100.0))).center());

    // let proxy = event_loop.create_proxy();
    // let _ = platform::start_toolbar(window.ns_window(), Mutex::new(proxy).into());

    {
        let sim = Arc::get_mut(&mut sim).unwrap();
        let in1 = sim.insert_component(Component::Input(High));
        let in2 = sim.insert_component(Component::Input(Low));

        let and_index = sim.insert_component(Component::XorGate);
        let output_index = sim.insert_component(Component::Output(Low));

        elements.push(Element {
            component: ComponentHandle(in1),
            position: (100.0, 100.0).into(),
        });

        elements.push(Element {
            component: ComponentHandle(in2),
            position: (100.0, 400.0).into(),
        });

        elements.push(Element {
            component: ComponentHandle(and_index),
            position: (300.0, 250.0).into(),
        });

        elements.push(Element {
            component: ComponentHandle(output_index),
            position: (500.0, 250.0).into(),
        });

        sim.connect(RegisteredPin(in1, 0), RegisteredPin(and_index, 0));
        sim.connect(RegisteredPin(in2, 0), RegisteredPin(and_index, 1));
        sim.connect(RegisteredPin(and_index, 2), RegisteredPin(output_index, 0));

        sim.tick();
    }

    let element_manager = ElementManager::new(sim.clone()).with_elements(elements);

    let mut render_manager = RenderManager::new(&window, element_manager).await;

    event_loop.run(move |event, _, cf| match event {
        Event::LoopDestroyed => *cf = ControlFlow::Exit,
        Event::WindowEvent { event, .. } => {
            render_manager.element_manager.event(&event);

            match &event {
                WindowEvent::CloseRequested => *cf = ControlFlow::Exit,
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => *cf = ControlFlow::Exit,
                WindowEvent::Resized(size) => render_manager.resize(size.width, size.height),
                _ => (),
            }
            window.request_redraw()
        }
        // Event::MainEventsCleared => window.request_redraw(),
        Event::RedrawRequested(_) => render_manager.draw(),
        _ => (),
    });
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <State as Debug>::fmt(self, f)
    }
}

struct SimParams<'a> {
    input: RegisteredPin,
    output: RegisteredPin,
    sim: &'a Simulator,
    pin_cache: &'a mut HashMap<RegisteredPin, State>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentHandle(usize);

impl ComponentHandle {
    pub fn to_pin(self, pin: usize) -> RegisteredPin {
        RegisteredPin(self.0, pin)
    }
}

enum Component {
    Input(State),
    Output(State),

    OrGate,
    AndGate,
    XorGate,
    NotGate,
}

impl Component {
    pub const fn input_len(&self) -> usize {
        match self {
            Component::Input(_) => 0,
            Component::Output(_) => 1,

            Component::OrGate => 2,
            Component::AndGate => 2,
            Component::XorGate => 2,
            Component::NotGate => 1,
        }
    }

    pub const fn output_len(&self) -> usize {
        match self {
            Component::Input(_) => 1,
            Component::Output(_) => 0,

            Component::OrGate => 1,
            Component::AndGate => 1,
            Component::XorGate => 1,
            Component::NotGate => 1,
        }
    }

    fn set_pin(&mut self, pin: usize, value: State) {
        match self {
            Component::Output(b) => *b = value,
            _ => panic!("Component doesn't have input!"),
        }
    }

    fn get_pin(&mut self, pin: usize) -> State {
        match self {
            Component::Input(b) => *b,
            _ => panic!("Component doesn't have output!"),
        }
    }

    fn set_input(&mut self, pin: usize, value: State) {
        match self {
            Component::Input(inp) => *inp = value,
            _ => panic!("Component isn't an input!"),
        }
    }

    fn inspect_pin(&mut self, pin: usize) -> State {
        match self {
            Component::Input(b) => *b,
            Component::Output(b) => *b,
            _ => panic!("component can't be inspected"),
        }
    }

    fn get_label(&self) -> &str {
        match self {
            Component::Input(_) => "input",
            Component::Output(_) => "output",
            Component::OrGate => "or gate",
            Component::AndGate => "and gate",
            Component::XorGate => "xor gate",
            Component::NotGate => "not gate",
        }
    }

    fn sim_pin(&mut self, pin: &RegisteredPin, params: &mut SimParams) -> State {
        params.pin_cache.get(pin).copied().unwrap_or_else(|| {
            let component = &params.sim.components[pin.0];
            let mut component = component.borrow_mut();

            let right_result = component.rev_sim(SimParams {
                input: *pin,
                output: RegisteredPin(params.input.0, 1),
                pin_cache: params.pin_cache,
                sim: params.sim,
            });

            right_result
        })
    }

    fn binary_sim(
        &mut self,
        left: &RegisteredPin,
        right: &RegisteredPin,
        params: &mut SimParams,
    ) -> Option<(State, State)> {
        let Some(left) = params.sim.out_to_in.get(&left) else {
            eprintln!(
                "WARNING: No connection for 'and gate' component, index: {:?}",
                left
            );
            return None;
        };

        let Some(right) = params.sim.out_to_in.get(&right) else {
            eprintln!(
                "WARNING: No connection for 'and gate' component, index: {:?}",
                right
            );
            return None;
        };

        let left = self.sim_pin(left, params);
        let right = self.sim_pin(right, params);

        Some((left, right))
    }

    #[inline]
    fn nsim<const N: usize>(
        &mut self,
        pins: [&RegisteredPin; N],
        params: &mut SimParams,
    ) -> Option<[State; N]> {
        let result = pins
            .into_iter()
            .map(|pin| {
                let Some(pin) = params.sim.out_to_in.get(pin) else {
                    eprintln!(
                        "WARNING: No connection for 'and gate' component, index: {:?}",
                        pin
                    );
                    return Invalid;
                };

                self.sim_pin(pin, params)
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Some(result)
    }

    fn rev_sim(&mut self, mut params: SimParams) -> State {
        match self {
            Component::Input(i) => {
                params.pin_cache.insert(params.output, *i);

                *i
            }
            Component::Output(o) => {
                assert_eq!(params.input.1, 0);

                let Some(next) = params.sim.out_to_in.get(&params.input) else {
                    eprintln!(
                        "WARNING: No connection for 'output' component, index: {:?}",
                        params.input
                    );
                    return State::Invalid;
                };

                if let Some(state) = params.pin_cache.get(next) {
                    return *state;
                }

                let component = &params.sim.components[next.0];
                let mut component = component.borrow_mut();

                let result = component.rev_sim(SimParams {
                    input: *next,
                    output: params.input,
                    pin_cache: params.pin_cache,
                    sim: params.sim,
                });
                *o = result;

                result
            }
            Component::OrGate => {
                let Some(result) = self.nsim(
                    [
                        &RegisteredPin(params.input.0, 0),
                        &RegisteredPin(params.input.0, 1),
                    ],
                    &mut params,
                ) else {
                    return Invalid;
                };

                match result {
                    [Invalid, _] | [_, Invalid] => Invalid,
                    [High, _] | [_, High] => High,
                    _ => Low,
                }
            }
            Component::AndGate => {
                let Some(result) = self.nsim(
                    [
                        &RegisteredPin(params.input.0, 0),
                        &RegisteredPin(params.input.0, 1),
                    ],
                    &mut params,
                ) else {
                    return Invalid;
                };

                match result {
                    [Invalid, _] | [_, Invalid] => Invalid,
                    [High, High] => High,
                    _ => Low,
                }
            }
            Component::XorGate => {
                let Some(result) = self.nsim(
                    [
                        &RegisteredPin(params.input.0, 0),
                        &RegisteredPin(params.input.0, 1),
                    ],
                    &mut params,
                ) else {
                    return Invalid;
                };

                match result {
                    [Invalid, _] | [_, Invalid] => Invalid,
                    [High, High] => Low,
                    [High, _] | [_, High] => High,
                    _ => Low,
                }
            }
            Component::NotGate => {
                let Some(result) = self.nsim([&RegisteredPin(params.input.0, 0)], &mut params)
                else {
                    return Invalid;
                };

                match result {
                    [Invalid] => Invalid,
                    [High] => Low,
                    [Low] => High,
                }
            }
        }
    }
}

/// Represents a registered pin in the simulator. First element is componenet index, second is pin index in component.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct RegisteredPin(usize, usize);

/// Registered one connection in the simulator
#[derive(Debug, Hash, PartialEq, Eq)]
struct Connection(RegisteredPin, RegisteredPin);

pub struct Simulator {
    /// Indicies into `components` field of input components
    input_components: Vec<usize>,
    /// Indicies into `components` field of output components
    output_components: Vec<usize>,

    /// Contains all of the components that the simlulator contains
    components: Vec<Rc<RefCell<Component>>>,

    /// Contains all of the connection mappings with inputs to outputs
    in_to_out: HashMap<RegisteredPin, RegisteredPin>,
    /// Contains all of the connection mappings with output to inputs
    out_to_in: HashMap<RegisteredPin, RegisteredPin>,
}

impl Simulator {
    fn new() -> Simulator {
        Simulator {
            input_components: Vec::new(),
            output_components: Vec::new(),
            components: Vec::new(),
            in_to_out: HashMap::new(),
            out_to_in: HashMap::new(),
        }
    }

    fn get_component(&self, handle: &ComponentHandle) -> &Rc<RefCell<Component>> {
        &self.components[handle.0]
    }

    fn insert_component(&mut self, component: Component) -> usize {
        let index = self.components.len();
        match component {
            Component::Input(_) => self.input_components.push(index),
            Component::Output(_) => self.output_components.push(index),
            _ => (),
        }

        self.components.push(Rc::new(RefCell::new(component)));

        index
    }

    fn connect(&mut self, input: RegisteredPin, output: RegisteredPin) {
        self.in_to_out.insert(input.clone(), output.clone());
        self.out_to_in.insert(output, input);
    }

    fn set_pin(&mut self, pin: RegisteredPin, value: State) {
        let component = &self.components[pin.0];
        let mut component = component.borrow_mut();

        component.set_pin(pin.1, value);
    }

    fn get_pin(&self, pin: RegisteredPin) -> State {
        let component = &self.components[pin.0];
        let mut component = component.borrow_mut();

        component.get_pin(pin.1)
    }

    fn set_input(&mut self, pin: RegisteredPin, value: State) {
        let component = &self.components[pin.0];
        let mut component = component.borrow_mut();

        component.set_input(pin.1, value);
    }

    fn inspect_pin(&self, pin: RegisteredPin) -> State {
        let component = &self.components[pin.0];
        let mut component = component.borrow_mut();

        component.inspect_pin(pin.1)
    }

    fn tick(&mut self) {
        let mut cache = HashMap::new();

        for output in &self.output_components {
            let component = &self.components[*output];
            let mut component = component.borrow_mut();

            component.rev_sim(SimParams {
                input: RegisteredPin(*output, 0),
                output: RegisteredPin(0, 0),
                sim: self,
                pin_cache: &mut cache,
            });
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_or() {
        let mut sim = Simulator::new();

        let in1 = sim.insert_component(Component::Input(Low));
        let in2 = sim.insert_component(Component::Input(Low));

        let or_index = sim.insert_component(Component::OrGate);
        let output_index = sim.insert_component(Component::Output(Low));

        sim.connect(RegisteredPin(in1, 0), RegisteredPin(or_index, 0));
        sim.connect(RegisteredPin(in2, 0), RegisteredPin(or_index, 1));
        sim.connect(RegisteredPin(or_index, 2), RegisteredPin(output_index, 0));

        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low
        );

        sim.set_input(RegisteredPin(in1, 0), Low);
        sim.set_input(RegisteredPin(in2, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.set_input(RegisteredPin(in2, 0), Low);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.set_input(RegisteredPin(in2, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );
    }

    #[test]
    fn test_and() {
        let mut sim = Simulator::new();

        let in1 = sim.insert_component(Component::Input(Low));
        let in2 = sim.insert_component(Component::Input(Low));

        let gate_index = sim.insert_component(Component::AndGate);
        let output_index = sim.insert_component(Component::Output(Low));

        sim.connect(RegisteredPin(in1, 0), RegisteredPin(gate_index, 0));
        sim.connect(RegisteredPin(in2, 0), RegisteredPin(gate_index, 1));
        sim.connect(RegisteredPin(gate_index, 2), RegisteredPin(output_index, 0));

        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low
        );

        sim.set_input(RegisteredPin(in1, 0), Low);
        sim.set_input(RegisteredPin(in2, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.set_input(RegisteredPin(in2, 0), Low);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.set_input(RegisteredPin(in2, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );
    }

    #[test]
    fn test_xor() {
        let mut sim = Simulator::new();

        let in1 = sim.insert_component(Component::Input(Low));
        let in2 = sim.insert_component(Component::Input(Low));

        let gate_index = sim.insert_component(Component::XorGate);
        let output_index = sim.insert_component(Component::Output(Low));

        sim.connect(RegisteredPin(in1, 0), RegisteredPin(gate_index, 0));
        sim.connect(RegisteredPin(in2, 0), RegisteredPin(gate_index, 1));
        sim.connect(RegisteredPin(gate_index, 2), RegisteredPin(output_index, 0));

        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low
        );

        sim.set_input(RegisteredPin(in1, 0), Low);
        sim.set_input(RegisteredPin(in2, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.set_input(RegisteredPin(in2, 0), Low);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.set_input(RegisteredPin(in2, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low
        );
    }

    #[test]
    fn test_not() {
        let mut sim = Simulator::new();

        let in1 = sim.insert_component(Component::Input(Low));
        let gate_index = sim.insert_component(Component::NotGate);
        let output_index = sim.insert_component(Component::Output(Low));

        sim.connect(RegisteredPin(in1, 0), RegisteredPin(gate_index, 0));
        sim.connect(RegisteredPin(gate_index, 2), RegisteredPin(output_index, 0));

        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            High
        );

        sim.set_input(RegisteredPin(in1, 0), High);
        sim.tick();
        assert_eq!(
            sim.inspect_pin(RegisteredPin(sim.output_components[0], 0)),
            Low,
        );
    }
}
