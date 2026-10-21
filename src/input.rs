//! Retain keyboard press edges even when press and release share one redraw.
use minifb::{InputCallback, Key, Window};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

#[derive(Default)]
struct State {
    held: HashSet<Key>,
    pressed: Vec<Key>,
}

#[derive(Clone, Default)]
pub struct Keyboard(Rc<RefCell<State>>);

impl InputCallback for Keyboard {
    fn add_char(&mut self, _codepoint: u32) {}
    fn set_key_state(&mut self, key: Key, down: bool) {
        let mut state = self.0.borrow_mut();
        if down {
            if state.held.insert(key) {
                state.pressed.push(key);
            }
        } else {
            state.held.remove(&key);
        }
    }
}

impl Keyboard {
    pub fn attach(window: &mut Window) -> Self {
        let keyboard = Self::default();
        window.set_input_callback(Box::new(keyboard.clone()));
        keyboard
    }
    pub fn take_pressed(&self) -> Vec<Key> {
        std::mem::take(&mut self.0.borrow_mut().pressed)
    }
    pub fn held(&self, key: Key) -> bool {
        self.0.borrow().held.contains(&key)
    }
    pub fn release_all(&self) {
        self.0.borrow_mut().held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_fast_tap_is_retained_and_auto_repeat_does_not_create_fire_edges() {
        let mut keyboard = Keyboard::default();
        keyboard.set_key_state(Key::A, true);
        keyboard.set_key_state(Key::A, true);
        keyboard.set_key_state(Key::A, false);
        assert_eq!(keyboard.take_pressed(), [Key::A]);
        assert!(!keyboard.held(Key::A));
        assert!(keyboard.take_pressed().is_empty());
    }
}
