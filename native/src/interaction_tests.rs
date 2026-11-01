//! Pointer regression tests dispatch actual press/release pairs, never UIA Invoke.
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Key, Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize};
use std::{cell::Cell, rc::Rc};

slint::slint! {
    import { Button, Toggle, Checkbox, Tabs, Dropdown, Slider, TextField } from "ui/components.slint";
    export component InteractionFixture inherits Window {
        width: 400px; height: 500px;
        in-out property <bool> controlled: false;
        in-out property <int> authoritative-index: 0;
        out property <int> displayed-index: choice.current-index;
        out property <int> tab-index: tabs.current;
        callback action(int);
        Button { x: 20px; y: 20px; width: 150px; height: 36px; text: "Action"; clicked => { root.action(0); } }
        Toggle { x: 20px; y: 80px; toggled => { root.action(1); } }
        Checkbox { x: 20px; y: 130px; width: 180px; height: 30px; text: "Check"; toggled => { root.action(2); } }
        tabs := Tabs { x: 20px; y: 180px; width: 220px; height: 38px; model: ["First", "Second"]; selected => { root.action(3); } }
        choice := Dropdown { x: 20px; y: 240px; width: 200px; height: 36px; model: ["First", "Second"]; controlled: root.controlled; current-index: root.authoritative-index; selected => { root.action(4); } }
        TextField { x: 20px; y: 310px; width: 200px; height: 36px; label: "Query"; }
        Slider { x: 20px; y: 380px; width: 200px; height: 30px; released => { root.action(5); } }
    }
}

struct TestPlatform;
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
    }
}
fn click(window: &slint::Window, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

#[test]
fn one_primary_click_dispatches_exactly_once_with_and_without_window_focus() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let fixture = InteractionFixture::new().unwrap();
    fixture.window().set_size(LogicalSize::new(400., 500.));
    fixture.show().unwrap();
    let counts = Rc::new([const { Cell::new(0) }; 6]);
    let observed = counts.clone();
    fixture
        .on_action(move |index| observed[index as usize].set(observed[index as usize].get() + 1));
    for active in [false, true, false, true] {
        fixture
            .window()
            .dispatch_event(WindowEvent::WindowActiveChanged(active));
        for (index, (x, y)) in [(60., 38.), (36., 90.), (28., 145.), (130., 198.)]
            .into_iter()
            .enumerate()
        {
            let before = counts[index].get();
            click(fixture.window(), x, y);
            assert_eq!(
                counts[index].get(),
                before + 1,
                "control {index}, active {active}"
            );
        }
    }
    click(fixture.window(), 55., 198.);
    let before_keys = counts[3].get();
    for (key, expected) in [
        (Key::RightArrow, 1),
        (Key::RightArrow, 0),
        (Key::LeftArrow, 1),
        (Key::Home, 0),
        (Key::End, 1),
    ] {
        fixture
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
        fixture
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
        assert_eq!(
            fixture.get_tab_index(),
            expected,
            "tab focus must follow repeated keyboard navigation"
        );
    }
    assert_eq!(counts[3].get(), before_keys + 5, "one selection per key");
    let before = counts[4].get();
    click(fixture.window(), 60., 258.);
    click(fixture.window(), 60., 332.);
    assert_eq!(
        counts[4].get(),
        before + 1,
        "dropdown opens and selects once"
    );
    let controlled = InteractionFixture::new().unwrap();
    controlled.window().set_size(LogicalSize::new(400., 500.));
    controlled.set_controlled(true);
    controlled.show().unwrap();
    let requests = Rc::new(Cell::new(0));
    let seen = requests.clone();
    controlled.on_action(move |i| {
        if i == 4 {
            seen.set(seen.get() + 1);
        }
    });
    click(controlled.window(), 60., 258.);
    click(controlled.window(), 60., 332.);
    assert_eq!(requests.get(), 1);
    assert_eq!(
        controlled.get_displayed_index(),
        0,
        "unacknowledged selection must not replace authoritative display"
    );
    controlled.set_authoritative_index(1);
    assert_eq!(
        controlled.get_displayed_index(),
        1,
        "backend acknowledgment must retain its binding"
    );
    let before = counts[5].get();
    click(fixture.window(), 100., 395.);
    assert_eq!(
        counts[5].get(),
        before + 1,
        "slider release must not also dispatch from clicked"
    );
    let app = crate::AppWindow::new().unwrap();
    app.window().set_size(LogicalSize::new(1120., 760.));
    app.show().unwrap();
    let navigations = Rc::new(Cell::new(0));
    let observed = navigations.clone();
    app.global::<crate::Nav>()
        .on_navigate(move |_| observed.set(observed.get() + 1));
    for active in [false, true] {
        app.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(active));
        for y in [100., 166., 232., 298., 364.] {
            let before = navigations.get();
            click(app.window(), 44., y);
            assert_eq!(
                navigations.get(),
                before + 1,
                "sidebar y {y}, active {active}"
            );
        }
    }
}

#[test]
fn instance_play_navigation_does_not_reborrow_ui_state() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let app = crate::AppWindow::new().unwrap();
    app.window().set_size(LogicalSize::new(1120., 760.));
    app.global::<crate::Shell>().set_loaded(true);
    app.show().unwrap();
    let root = std::env::temp_dir().join(format!("aurora-navigation-test-{}", std::process::id()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let host = aurora_launcher_lib::host::Host::native(
        Ok(root),
        "1.4.2",
        runtime.handle().clone(),
        |_, _| {},
    );
    crate::ui::install(app.clone_strong(), host, runtime.handle().clone());
    crate::bind_navigation(&app);
    crate::instances::bind(&app);
    crate::ui::with(|window, state| {
        state.workspace = Some(crate::instances::Workspace {
            id: "navigation-fixture".into(),
            ..Default::default()
        });
        window
            .global::<crate::Nav>()
            .set_page(crate::Page::Instances);
        window.global::<crate::Nav>().set_workspace_open(true);
    });
    click(app.window(), 1050., 60.);
    assert_eq!(app.global::<crate::Nav>().get_page(), crate::Page::Home);
    assert!(
        !crate::ui::state(|state| state.play_busy),
        "an invalid instance must not launch a child"
    );
    app.global::<crate::Instances>()
        .set_memory_mib_text("8192".into());
    app.global::<crate::Instances>().set_config_dirty(true);
    crate::instances::show_settings("navigation-fixture".into());
    assert_eq!(
        app.global::<crate::Instances>()
            .get_memory_mib_text()
            .as_str(),
        "8192",
        "opening settings must retain the memory draft"
    );
    assert!(app.global::<crate::Instances>().get_config_dirty());
    assert_eq!(
        app.global::<crate::Instances>().get_tab(),
        crate::instances::TAB_SETTINGS
    );
    crate::ui::release();
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
}
