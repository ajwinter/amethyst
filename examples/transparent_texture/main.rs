//! Demonstrates how to load renderable objects, along with several lighting
//! methods.
//!
//! TODO: Rewrite for new renderer.

use amethyst;

use amethyst::{
    assets::{
        Completion, Handle, HotReloadBundle, Prefab, PrefabLoader, PrefabLoaderSystem,
        ProgressCounter, RonFormat, AssetLoaderSystemData,
    },
    core::{
        nalgebra::{UnitQuaternion, Vector3},
        timing::Time,
        transform::{Transform, TransformBundle},
    },
    ecs::prelude::{Entity, Join, Read, ReadStorage, System, Write, WriteStorage},
    input::InputBundle,
    prelude::*,
    renderer::*,
    ui::{DrawUi, UiBundle, UiCreator, UiFinder, UiText},
    utils::{
        application_root_dir,
        fps_counter::{FPSCounter, FPSCounterBundle},
        scene::BasicScenePrefab,
    },
    Error,
};

type MyPrefabData = BasicScenePrefab<Vec<PosNormTex>>;

#[derive(Default)]
struct Loading {
    progress: ProgressCounter,
    prefab: Option<Handle<Prefab<MyPrefabData>>>,
}

struct Example {
    scene: Handle<Prefab<MyPrefabData>>,
}

impl SimpleState for Loading {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        self.prefab = Some(data.world.exec(|loader: PrefabLoader<'_, MyPrefabData>| {
            loader.load("prefab/renderable.ron", RonFormat, (), &mut self.progress)
        }));

        data.world.exec(|mut creator: UiCreator<'_>| {
            creator.create("ui/fps.ron", &mut self.progress);
            creator.create("ui/loading.ron", &mut self.progress);
        });
    }

    fn update(&mut self, data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        match self.progress.complete() {
            Completion::Failed => {
                println!("Failed loading assets: {:?}", self.progress.errors());
                Trans::Quit
            }
            Completion::Complete => {
                println!("Assets loaded, swapping state");
                if let Some(entity) = data
                    .world
                    .exec(|finder: UiFinder<'_>| finder.find("loading"))
                {
                    let _ = data.world.delete_entity(entity);
                }
                Trans::Switch(Box::new(Example {
                    scene: self.prefab.as_ref().unwrap().clone(),
                }))
            }
            Completion::Loading => Trans::None,
        }
    }
}

impl SimpleState for Example {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let StateData { world, .. } = data;

        world.create_entity().with(self.scene.clone()).build();

        // Mesh with transparent texture
        let mat_defaults = world.read_resource::<MaterialDefaults>().0.clone();
        let (mesh, albedo) = {
            let mesh = world.exec(|loader: AssetLoaderSystemData<'_, Mesh>| {
                loader.load_from_data(
                    Shape::Plane(None).generate::<Vec<PosNormTex>>(None),
                    (),
                )
            });
            let albedo = world.exec(|loader: AssetLoaderSystemData<'_, Texture>| {
                loader.load(
                    "texture/logo_transparent.png",
                    PngFormat,
                    TextureMetadata::srgb(),
                    (),
                )
            });

            (mesh, albedo)
        };
        let mtl = Material {
            albedo,
            ..mat_defaults.clone()
        };
        let mut transform = Transform::default();
        transform.set_xyz(-5.0, -5.0, 5.0);
        transform.set_scale(8.0, 8.0, 8.0);
        world
            .create_entity()
            .with(transform)
            .with(mesh.clone())
            .with(mtl)
            .with(Transparent)
            .build();
    }
}

fn main() -> Result<(), Error> {
    amethyst::start_logger(Default::default());

    let app_root = application_root_dir()?;

    // Add our meshes directory to the asset loader.
    let resources_directory = app_root.join("examples/assets/");

    let display_config_path = app_root.join("examples/renderable/resources/display_config.ron");

    let pipe = Pipeline::build().with_stage(
        Stage::with_backbuffer()
            .clear_target([0.0, 0.0, 0.0, 1.0], 1.0)
            .with_pass(DrawShaded::<PosNormTex>::new()
                .with_transparency(ColorMask::all(), ALPHA, Some(DepthMode::LessEqualWrite)))
            .with_pass(DrawUi::new()),
    );

    let game_data = GameDataBuilder::default()
        .with(PrefabLoaderSystem::<MyPrefabData>::default(), "", &[])
        .with::<ExampleSystem>(ExampleSystem::default(), "example_system", &[])
        .with_bundle(TransformBundle::new().with_dep(&["example_system"]))?
        .with_bundle(UiBundle::<String, String>::new())?
        .with_bundle(HotReloadBundle::default())?
        .with_bundle(FPSCounterBundle::default())?
        .with_bundle(
            RenderBundle::new(pipe, Some(DisplayConfig::load(display_config_path)))
                .with_sprite_visibility_sorting(&["transform_system"])
        )?
        .with_bundle(InputBundle::<String, String>::new())?;
    let mut game = Application::build(resources_directory, Loading::default())?.build(game_data)?;
    game.run();
    Ok(())
}

struct DemoState {
    camera_angle: f32,
}

impl Default for DemoState {
    fn default() -> Self {
        DemoState {
            camera_angle: 0.0,
        }
    }
}

#[derive(Default)]
struct ExampleSystem {
    fps_display: Option<Entity>,
}

impl<'a> System<'a> for ExampleSystem {
    type SystemData = (
        Read<'a, Time>,
        ReadStorage<'a, Camera>,
        WriteStorage<'a, Transform>,
        Write<'a, DemoState>,
        WriteStorage<'a, UiText>,
        Read<'a, FPSCounter>,
        UiFinder<'a>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (time, camera, mut transforms, mut state, mut ui_text, fps_counter, finder) =
            data;
        let camera_angular_velocity = 0.1;

        state.camera_angle += camera_angular_velocity * time.delta_seconds();

        let delta_rot = UnitQuaternion::from_axis_angle(
            &Vector3::z_axis(),
            camera_angular_velocity * time.delta_seconds(),
        );
        for (_, transform) in (&camera, &mut transforms).join() {
            // Append the delta rotation to the current transform.
            *transform.isometry_mut() = delta_rot * transform.isometry();
        }

        if let None = self.fps_display {
            if let Some(fps_entity) = finder.find("fps_text") {
                self.fps_display = Some(fps_entity);
            }
        }
        if let Some(fps_entity) = self.fps_display {
            if let Some(fps_display) = ui_text.get_mut(fps_entity) {
                if time.frame_number() % 20 == 0 {
                    let fps = fps_counter.sampled_fps();
                    fps_display.text = format!("FPS: {:.*}", 2, fps);
                }
            }
        }
    }
}
