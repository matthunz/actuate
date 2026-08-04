# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## Breaking changes

- The `ui` feature now enables `ecs` and `picking`, and pulls in `bevy_color`, `bevy_input`,
  `bevy_input_focus`, and `bevy_text`. It previously only enabled `bevy_ui`, which wasn't enough
  to build `src/ui` on its own.

## Features

- Add `text_input` composable

## [0.22.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.21.0...actuate-v0.22.0) - 2026-8-2

## Breaking changes

- Update to Bevy v0.19

## [0.21.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.20.1...actuate-v0.21.0) - 2025-12-18

## Breaking changes

- Update to Bevy v0.17.3

## [0.20.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.20.0...actuate-v0.20.1) - 2024-12-13

## Documentation

- Fix formatting in crate example

## [0.20.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.19.1...actuate-v0.20.0) - 2024-12-13

## Breaking changes

- Fix `Data` impl for `&T` (63d89ed)
- Remove Data from root exports (677b160)
- Replace `Memoize` trait with more specific `Generational` trait (febe238)

## Features

- Impl `Data` for Rc<dyn Fn(..)> and derive `Clone` for `Catch` composable (e038307)
- Impl `Clone` for `from_fn`, `from_iter`, and `memo` composables (21c016f)
- Add `material_ui` composable (5dad9a3)

## Fixes

- Replace `std` deps with `core` (68d44a2)
- Simplify styling in `scroll_view` composable and update `http` example (f90e4c4)
- Check for removed entities in Spawn composable (c23e158)

## Documentation

- Add docs to `use_local_task` (63d89ed)
- Add docs to `use_task` (7ddbe84)
- Update counter example (3b79bb1)
- Update borrowing docs (efcdfe3)

## [0.19.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.19.0...actuate-v0.19.1) - 2024-12-09

## Features

- Add `use_effect` hook (5ae0a51)
  - `fn use_effect<D, T>(cx: ScopeState<'_>, dependency: D, effect: impl FnOnce(&D))`

## Fixes

- Remove `AnyItemState` in `from_iter` composable to pass stacked borrows check in miri (2360814)

## Documentation

- Add docs for `from_fn` and `from_iter` composables (5c379e1)

## [0.19.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.18.1...actuate-v0.19.0) - 2024-12-08

## Breaking changes

- Require `'static` items in `from_iter` composable
  - This prevents edge cases where an item may have been removed from a collection, but references still exist down the tree.

## Documentation

- Add logo to rustdocs (702b1e0)
- Update material docs (3084286)
- Update link to `core::error::Error` in `Catch` composable (b664206)

## [0.18.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.18.0...actuate-v0.18.1) - 2024-12-07

## Fixes

- Specify `winit` backend for `docs.rs` (62bec2d)

## [0.18.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.17.2...actuate-v0.18.0) - 2024-12-07

## Breaking changes

- Create `ScrollView` composable and new `ui` module (28628f4, ebb17b0)
  - The `material` is now located under `ui`

## Features

- Add support for reactive Bevy desktop apps (7c65ba9, 6918000)
- Add more picking handler methods to `Modify` (64404b3)
- More advanced typography with new Text composable (825e007)
- Derive `Clone` for `Spawn` and refactor (6c4e457)
- Add methods to modify all Node fields

## [0.17.2](https://github.com/actuate-rs/actuate/compare/actuate-v0.17.1...actuate-v0.17.2) - 2024-12-07

## Fixes

- Update pending composable ordering and track child index in `Spawn` composable (fdf89ed)
- Reverse node IDs and refactor internals (42e1971)

## Documentation

- Add docs to `spawn` constructor and split up ecs module (9c06bfe)
- Move examples for `catch` and `dyn_compose` to rustdocs (9502a4b)
- Move traits example to data module and add docs, reorganize examples (67ec922)
- Update Data docs (829c6d9)

## [0.17.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.17.0...actuate-v0.17.1) - 2024-12-06

## Features

- Create `FromFn` composable

  - You can now create a composable without input using `from_fn` (433ab1d)
  - ```rs
     fn from_fn<F, C>(f: F) -> FromFn<F, C>
     where
         F: Fn(ScopeState) -> C,
         C: Compose
    ```

- Derive `Clone` and `Debug` for `Button`, `Container`, and `RadioButton` (4f337ed)

## Documentation

- Update docs for feature flags (869aa89)

## [0.17.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.16.1...actuate-v0.17.0) - 2024-12-06

## Breaking changes

- Move `Modifier` and `Modify` to ecs module (behind new picking feature) (35b10ea)
  - These items can be useful for other design systems than Material 3
- Call `on_insert` on every insertion of a spawned bundle (this now requires `Fn` instead of `FnOnce`) (533da07)

## Fixes

- Revert from breadth-first traversal of the composition to depth-first (8b3acd2)
- Update styling for `Container` (9cca3a7)

## [0.16.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.16.0...actuate-v0.16.1) - 2024-12-05

## Features

- Material UI components

  - `Button`
  - `Container`
  - `RadioButton`
  - `text`
    - `label`
    - `heading`

- New scheduling algorithm based on `BTreeSet` (2a457a9)

## [0.16.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.15.0...actuate-v0.16.0) - 2024-12-05

### Breaking changes

- Major internal rewrite! (9ef73eb) The new internals allow for more dynamic control over the composition
  , enabling features like pause and resume of a composition.
  `Composer::try_compose` will now also skip directly to changed composables, rather than setting change flags.
  - Removes exported methods for `ScopeData`
  - The `Runtime` struct is now private to ensure safety

## Features

- `Composer` is now an iterator! This allows for stepping through each composable in the composition.
- `Composer` also implements `fmt::Debug`:

  ```rs
  use actuate::prelude::*;
  use actuate::composer::Composer;

  #[derive(Data)]
  struct A;

  impl Compose for A {
      fn compose(cx: Scope<Self>) -> impl Compose {
          (B, C)
      }
  }

  #[derive(Data)]
  struct B;

  impl Compose for B {
      fn compose(cx: Scope<Self>) -> impl Compose {}
  }

  #[derive(Data)]
  struct C;

  impl Compose for C {
      fn compose(cx: Scope<Self>) -> impl Compose {}
  }

  let mut composer = Composer::new(A);
  composer.try_compose().unwrap();

  assert_eq!(format!("{:?}", composer), "Composer(A(B, C))")
  ```

## [0.15.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.14.2...actuate-v0.15.0) - 2024-12-03

### Breaking changes

- Add `#[actuate(path = "..")]` attribute to `Data` macro and use fully-qualified path to Actuate by default (b159478).
  - This allows for use of the `Data` macro without importing the full `prelude`.
- Replace `DynCompose::new` with `dyn_compose` constructor fn (9d65ec8).
- Return `Rc` from use_context
  - `fn use_context<T: 'static>(cx: ScopeState<'_>) -> Result<&Rc<T>, ContextError<T>> { .. }`
  - This allows for cloning context into `'static` environments.

### Refactors

- Use explicit imports internally to speed up compile times and exclude hidden `Data` traits from prelude (07bfd96).

## [0.14.2](https://github.com/actuate-rs/actuate/compare/actuate-v0.14.1...actuate-v0.14.2) - 2024-12-03

### Features

- Optimize empty composables by skipping creation of ScopeData

### Fixes

- Enable Tokio dependency with animation and ecs features (5263fe4)

## [0.14.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.14.0...actuate-v0.14.1) - 2024-12-03

### Fixes

- Remove unused tokio read lock guard (0ad962f)

## [0.14.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.13.0...actuate-v0.14.0) - 2024-12-03

### Breaking Changes

- Remove unsound `Compose` impl for `Map` and create `MapUnchecked` struct
  - The original `Compose` impl for `Map` would cause undefined behavior if multiple references to the same composable were used. The new unsafe `MapUnchecked` keeps this functionality for low-level components, where the documented safety contract can be checked. However, for most composables I now see `Compose + Clone` being a typical pattern (which I think is fine given some composables only copy references when cloned, and references to composables can still be passed around).

### Fixes

- Impl re-composition when the type has changed in `DynCompose` (7d41100)

### Documentation

- Update docs for `Spawn` composable (205b88a)
- Add example to showcase `DynCompose` (7d41100)

## [0.13.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.12.0...actuate-v0.13.0) - 2024-12-02

### Breaking Changes

- Use `PartialEq` in `use_memo` instead of the `Memoize` trait (6539c95)
  - This is to memoize tuples and other groups of data.
    To use pointer equality, you can still use `Signal::generation` or `Memoize::memoize` to get the current generation.
- Remove unused UseWorld struct (81615cd)

### Documentation

- Add more documentation to the `Catch` composable
  - Adds a quick explanation of using `Result` + `Catch`, and links to the `catch` constructor function for more info.
- Add explanation to `compose::from_iter` (dc6715d)

### Other

- Change release procedure and update CI (dd4be8d, fe23aad, 723fe6c)

## [0.12.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.11.0...actuate-v0.12.0) - 2024-12-02

### Other

- `#![no_std]` support ([#100](https://github.com/actuate-rs/actuate/pull/100))
- Clean up and add internal docs
- Remove Sized bound in Compose trait
- Create `Catch` composable and impl `Compose` for `Result` ([#99](https://github.com/actuate-rs/actuate/pull/99))
- Add getter and setter methods to ScopeData
- Update docs
- Remove is_empty from ScopeState in favor of checking for empty types
- Create README.md

## [0.11.0](https://github.com/actuate-rs/actuate/compare/actuate-v0.10.2...actuate-v0.11.0) - 2024-11-29

### Other

- Update to Bevy 0.15.0
- Disable observers after drop
- Add support for standard references in RefMap and Cow
- Fix formatting in README

## [0.10.2](https://github.com/actuate-rs/actuate/compare/actuate-v0.10.1...actuate-v0.10.2) - 2024-11-28

### Other

- Add specialized impl of SystemParamFunction for Triggers
- Export animation channel
- Impl Data for UseAnimated
- Impl Data for Pin
- Impl Data for Box<dyn Future<Output = ()>>
- Allow return values for Data fns
- Create `use_animated` hook ([#88](https://github.com/actuate-rs/actuate/pull/88))
- Fix tasks not running on the ecs

## [0.10.1](https://github.com/actuate-rs/actuate/compare/actuate-v0.10.0...actuate-v0.10.1) - 2024-11-26

### Other

- Apply system params in use_world_once
- Apply deferred system param updates
- Add SignalMut::set_if_neq and generation methods
