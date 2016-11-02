#![feature(proc_macro)]
extern crate descartes;
#[macro_use]
pub extern crate glium;
extern crate kay;
#[macro_use]
extern crate kay_macros;
extern crate glium_text;

pub use ::descartes::{P3, V3, Iso3, Persp3, ToHomogeneous, Norm, Into2d, Into3d, WithUniqueOrthogonal};
use ::kay::{ID, World, Recipient, CVec, ActorSystem, Individual};
use std::collections::HashMap;

use glium::{index, Surface};
pub use glium::backend::glutin_backend::GlutinFacade;

#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 3]
}
implement_vertex!(Vertex, position);

#[derive(Copy, Clone)]
pub struct Instance {
    pub instance_position: [f32; 3],
    pub instance_direction: [f32; 2],
    pub instance_color: [f32; 3]
}
implement_vertex!(Instance, instance_position, instance_direction, instance_color);

pub struct Eye {
    pub position: P3,
    pub target: P3,
    pub up: V3,
    pub field_of_view: f32
}

#[derive(Compact)]
pub struct Thing {
    vertices: CVec<Vertex>,
    indices: CVec<u16>
}

impl Thing {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Thing {
        Thing{vertices: vertices.into(), indices: indices.into()}
    }
}

impl Clone for Thing {
    fn clone(&self) -> Thing {
        Thing {
            vertices: self.vertices.to_vec().into(),
            indices: self.indices.to_vec().into()
        }
    }
}

pub struct Batch {
    prototype: Thing,
    pub instances: Vec<Instance>
}

impl Batch {
    pub fn new(prototype: Thing, instances: Vec<Instance>) -> Batch {
        Batch{prototype: prototype, instances: instances}
    }
}

pub struct Scene {
    pub eye: Eye,
    pub batches: HashMap<usize, Batch>,
    pub things: HashMap<usize, (Thing, Instance)>,
    pub renderables: Vec<ID>,
    pub debug_text: String
}

impl Scene {
    pub fn new() -> Scene {
        Scene{
            eye: Eye{
                position: P3::new(-5.0, -5.0, 5.0),
                target: P3::new(0.0, 0.0, 0.0),
                up: V3::new(0.0, 0.0, 1.0),
                field_of_view: 0.3 * ::std::f32::consts::PI
            },
            batches: HashMap::new(),
            things: HashMap::new(),
            renderables: Vec::new(),
            debug_text: String::new()
        }
    }
}

pub struct Renderer {
    pub scenes: HashMap<usize, Scene>,
    pub render_context: RenderContext
}

impl Individual for Renderer {}

#[derive(Copy, Clone)]
pub enum Control {
    Setup,
    Render,
    Submit
}

#[derive(Copy, Clone)]
pub struct SetupInScene {
    pub renderer_id: ID,
    pub scene_id: usize
}
#[derive(Copy, Clone)]
pub struct RenderToScene {
    pub renderer_id: ID,
    pub scene_id: usize
}

#[derive(Copy, Clone)]
pub struct MoveEye {
    pub scene_id: usize,
    pub delta: V3
}

#[derive(Compact)]
pub struct AddBatch {scene_id: usize, batch_id: usize, thing: Thing}

impl AddBatch {
    pub fn new(scene_id: usize, batch_id: usize, thing: Thing) -> AddBatch {
        AddBatch{scene_id: scene_id, batch_id: batch_id, thing: thing}
    }
}

#[derive(Compact)]
pub struct UpdateThing {scene_id: usize, thing_id: usize, thing: Thing, instance: Instance}

impl UpdateThing {
    pub fn new(scene_id: usize, thing_id: usize, thing: Thing, instance: Instance) -> UpdateThing {
        UpdateThing{scene_id: scene_id, thing_id: thing_id, thing: thing, instance: instance}
    }
}

#[derive(Copy, Clone)]
pub struct AddInstance {
    pub scene_id: usize,
    pub batch_id: usize,
    pub position: Instance
}

impl Recipient<Control> for Renderer {
    fn react_to(&mut self, msg: &Control, world: &mut World, self_id: ID) {match msg {
        &Control::Setup => {
            for (scene_id, scene) in &self.scenes {
                for renderable in &scene.renderables {
                    world.send(*renderable, SetupInScene{renderer_id: self_id, scene_id: *scene_id});
                }
            }
        },

        &Control::Render => {
            for (scene_id, mut scene) in &mut self.scenes {
                for batch in (&mut scene).batches.values_mut() {
                    batch.instances.clear();
                }
                for renderable in &scene.renderables {
                    world.send(*renderable, RenderToScene{renderer_id: self_id, scene_id: *scene_id});
                }
            }
        }

        &Control::Submit => {
            for scene in self.scenes.values() {
                self.render_context.submit(scene);
            }
        }
    }}
}

impl Recipient<AddBatch> for Renderer {
    fn receive(&mut self, msg: &AddBatch) {match msg {
        &AddBatch{scene_id, batch_id, ref thing} => {
            self.scenes.get_mut(&scene_id).unwrap().batches.insert(batch_id, Batch::new(thing.clone(), Vec::new()));
        }
    }}
}

impl Recipient<AddInstance> for Renderer {
    fn receive(&mut self, msg: &AddInstance) {match msg {
        &AddInstance{scene_id, batch_id, position} => {
            self.scenes.get_mut(&scene_id).unwrap().batches.get_mut(&batch_id).unwrap().instances.push(position);
        }
    }}
}

impl Recipient<UpdateThing> for Renderer {
    fn receive(&mut self, msg: &UpdateThing) {match msg {
        &UpdateThing{scene_id, thing_id, ref thing, instance} => {
            self.scenes.get_mut(&scene_id).unwrap().things.insert(thing_id, (thing.clone(), instance));
        }
    }}
}

impl Recipient<MoveEye> for Renderer {
    fn receive(&mut self, msg: &MoveEye) {match msg{
        &MoveEye{scene_id, delta} => {
            let ref mut eye = self.scenes.get_mut(&scene_id).unwrap().eye;
            let eye_direction_2d = (eye.target - eye.position).into_2d().normalize();
            let absolute_delta = delta.x * eye_direction_2d.into_3d()
                + delta.y * eye_direction_2d.orthogonal().into_3d()
                + V3::new(0.0, 0.0, delta.z);
            eye.position += absolute_delta;
            eye.target += absolute_delta;
        }
    }}
}

impl Renderer {
    pub fn new (window: GlutinFacade) -> Renderer {
        Renderer {
            scenes: HashMap::new(),
            render_context: RenderContext::new(window)
        }
    }
}

pub fn setup(system: &mut ActorSystem, renderer: Renderer) {
    system.add_individual(renderer);
    system.add_individual_inbox::<Control, Renderer>();
    system.add_individual_inbox::<AddBatch, Renderer>();
    system.add_individual_inbox::<AddInstance, Renderer>();
    system.add_individual_inbox::<UpdateThing, Renderer>();
    system.add_individual_inbox::<MoveEye, Renderer>();

    system.world().send_to_individual::<Renderer, _>(Control::Setup);
}

pub struct RenderContext {
    pub window: GlutinFacade,
    batch_program: glium::Program,
    text_system: glium_text::TextSystem,
    font: glium_text::FontTexture
}

impl RenderContext {
    pub fn new (window: GlutinFacade) -> RenderContext {
        RenderContext{
            batch_program: program!(&window,
                140 => {
                    vertex: include_str!("shader/solid_140.glslv"),
                    fragment: include_str!("shader/solid_140.glslf")
                }
            ).unwrap(),
            text_system: glium_text::TextSystem::new(&window),
            font: glium_text::FontTexture::new(
                &window,
                ::std::fs::File::open(&::std::path::Path::new("fonts/ClearSans-Regular.ttf")).unwrap(),
                64
            ).unwrap(),
            window: window,
        }
    }

    pub fn submit (&self, scene: &Scene) {
        let mut target = self.window.draw();

        let view : [[f32; 4]; 4] = *Iso3::look_at_rh(
            &scene.eye.position,
            &scene.eye.target,
            &scene.eye.up
        ).to_homogeneous().as_ref();
        let perspective : [[f32; 4]; 4] = *Persp3::new(
            target.get_dimensions().0 as f32 / target.get_dimensions().1 as f32,
            scene.eye.field_of_view,
            0.1,
            1000.0
        ).to_matrix().as_ref();
        
        let uniforms = uniform! {
            view: view,
            perspective: perspective
        };

        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::draw_parameters::DepthTest::IfLess,
                write: true,
                .. Default::default()
            },
            .. Default::default()
        };
        
        // draw a frame
        target.clear_color_and_depth((1.0, 1.0, 1.0, 1.0), 1.0);

        for batch in scene.batches.values() {
            let vertices = glium::VertexBuffer::new(&self.window, &batch.prototype.vertices).unwrap();
            let indices = glium::IndexBuffer::new(&self.window, index::PrimitiveType::TrianglesList, &batch.prototype.indices).unwrap();
            let instances = glium::VertexBuffer::dynamic(&self.window, batch.instances.as_slice()).unwrap();
            target.draw((&vertices, instances.per_instance().unwrap()), &indices, &self.batch_program, &uniforms, &params).unwrap();
        }

        for &(ref thing, instance) in scene.things.values() {
            let vertices = glium::VertexBuffer::new(&self.window, &thing.vertices).unwrap();
            let indices = glium::IndexBuffer::new(&self.window, index::PrimitiveType::TrianglesList, &thing.indices).unwrap();
            let instances = glium::VertexBuffer::new(&self.window, &[instance]).unwrap();
            target.draw((&vertices, instances.per_instance().unwrap()), &indices, &self.batch_program, &uniforms, &params).unwrap();
        }

        let text = glium_text::TextDisplay::new(&self.text_system, &self.font, scene.debug_text.as_str());
        let text_matrix = [
            [0.05, 0.0, 0.0, 0.0],
            [0.0, 0.05, 0.0, 0.0],
            [0.0, 0.0, 0.05, 0.0],
            [-0.9, 0.8, 0.0, 1.0f32]
        ];

        glium_text::draw(&text, &self.text_system, &mut target, text_matrix, (0.0, 0.0, 0.0, 1.0));

        target.finish().unwrap();
    }
}