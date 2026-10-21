//! A very small animation store.
//!
//! The interface needs one thing from animation: a value that eases toward a
//! target instead of jumping. Controls use it for hover, for press, for the
//! view switch, and for the section change. It costs one number per control,
//! and nothing animates while the app is idle.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct Tween {
    /// Where the control is now.
    value: f32,
    /// Where it is going.
    target: f32,
    /// Where it came from, so a change mid-flight never snaps.
    from: f32,
    start: Instant,
}

impl Tween {
    fn settled(target: f32) -> Self {
        Self {
            value: target,
            target,
            from: target,
            start: Instant::now(),
        }
    }
}

/// One control, with a channel for hover and a channel for press.
#[derive(Debug, Clone, Copy)]
struct Control {
    hover: Tween,
    press: Tween,
}

impl Control {
    fn new() -> Self {
        Self {
            hover: Tween::settled(0.0),
            press: Tween::settled(0.0),
        }
    }
}

#[derive(Debug, Default)]
pub struct Motion {
    controls: Vec<(&'static str, Control)>,
    duration: Duration,
}

impl Motion {
    pub fn new(duration: Duration) -> Self {
        Self {
            controls: Vec::new(),
            duration,
        }
    }

    fn control(&self, key: &str) -> Option<&Control> {
        self.controls
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, control)| control)
    }

    /// The hover state of a control, where 0 is at rest and 1 is hovered.
    pub fn hover(&self, key: &str) -> f32 {
        self.control(key)
            .map(|control| control.hover.value)
            .unwrap_or(0.0)
    }

    /// The press state of a control, where 0 is at rest and 1 is pressed.
    pub fn press(&self, key: &str) -> f32 {
        self.control(key)
            .map(|control| control.press.value)
            .unwrap_or(0.0)
    }

    /// Ease a control's hover toward a target. The value continues from where
    /// it is, so a change in the middle of a transition never snaps.
    pub fn set_hover(&mut self, key: &'static str, target: f32) {
        let now = Instant::now();
        let control = self.entry(key);
        if (control.hover.target - target).abs() < f32::EPSILON {
            return;
        }
        control.hover.from = control.hover.value;
        control.hover.target = target;
        control.hover.start = now;
    }

    /// Ease a control's press state toward a target.
    pub fn set_press(&mut self, key: &'static str, target: f32) {
        let now = Instant::now();
        let control = self.entry(key);
        if (control.press.target - target).abs() < f32::EPSILON {
            return;
        }
        control.press.from = control.press.value;
        control.press.target = target;
        control.press.start = now;
    }

    /// Start a control from zero and ease it to a target. Used when a section
    /// changes and its content should fade in.
    pub fn restart(&mut self, key: &'static str, target: f32) {
        let now = Instant::now();
        let control = self.entry(key);
        control.hover = Tween {
            value: 0.0,
            target,
            from: 0.0,
            start: now,
        };
    }

    fn entry(&mut self, key: &'static str) -> &mut Control {
        match self.controls.iter().position(|(name, _)| *name == key) {
            Some(index) => &mut self.controls[index].1,
            None => {
                self.controls.push((key, Control::new()));
                &mut self.controls.last_mut().expect("just pushed").1
            }
        }
    }

    /// Move every control that is still easing. Returns true when something
    /// changed, so the window knows it has to repaint.
    pub fn advance(&mut self) -> bool {
        let now = Instant::now();
        let mut changed = false;
        for (_, control) in self.controls.iter_mut() {
            for tween in [&mut control.hover, &mut control.press] {
                if (tween.value - tween.target).abs() < f32::EPSILON {
                    continue;
                }
                let progress = if self.duration.is_zero() {
                    1.0
                } else {
                    let elapsed = now.saturating_duration_since(tween.start).as_secs_f32();
                    (elapsed / self.duration.as_secs_f32()).min(1.0)
                };
                tween.value = tween.from + (tween.target - tween.from) * ease_out(progress);
                if progress >= 1.0 {
                    tween.value = tween.target;
                }
                changed = true;
            }
        }
        changed
    }
}

/// A quick, settled curve. It arrives without a long tail, so the interface
/// never feels like it is waiting for an animation to finish.
fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}
