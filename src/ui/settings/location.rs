use crate::Coordinates;
use crate::platform::location::Request;
use std::io;
use windows_reactor::Button;
use windows_reactor::Callback;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::StackPanel;
use windows_reactor::TextBlock;
use windows_reactor::TextWrapping;
use windows_reactor::View;
use windows_reactor::ViewContext;

#[derive(Clone, PartialEq)]
pub(super) struct Input {
    pub(super) enabled: bool,
    pub(super) epoch: u64,
    pub(super) completed: Callback<(u64, io::Result<Coordinates>)>,
}

pub(super) struct Location {
    input: Input,
    generation: u64,
    active: Option<u64>,
    request: Option<Request>,
}

pub(super) enum Message {
    Start,
    Cancel,
    Access(u64, io::Result<()>),
    Position(u64, io::Result<Coordinates>),
}

impl Component for Location {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            generation: 0,
            active: None,
            request: None,
        }
    }

    fn input_changed(&mut self, input: &Input, _: &ComponentContext<Self>) {
        if input.epoch != self.input.epoch || !input.enabled {
            self.cancel();
        }
        self.input.clone_from(input);
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::Start if self.input.enabled && self.active.is_none() => {
                self.generation = self.generation.wrapping_add(1);
                let generation = self.generation;
                self.active = Some(generation);
                let completion = context.completion();
                match Request::access(move |result| {
                    let _delivered = completion
                        .clone()
                        .complete(Message::Access(generation, result));
                }) {
                    Ok(request) => self.request = Some(request),
                    Err(error) => self.finish(generation, Err(error)),
                }
            }
            Message::Cancel => self.cancel(),
            Message::Access(generation, result) if self.active == Some(generation) => {
                self.request = None;
                if let Err(error) = result {
                    self.finish(generation, Err(error));
                    return;
                }
                let completion = context.completion();
                match Request::position(move |result| {
                    let _delivered = completion
                        .clone()
                        .complete(Message::Position(generation, result));
                }) {
                    Ok(request) => self.request = Some(request),
                    Err(error) => self.finish(generation, Err(error)),
                }
            }
            Message::Position(generation, result) => self.finish(generation, result),
            _ => {}
        }
    }

    fn view(&self, input: &Input, context: &mut ViewContext<'_, Self>) -> View {
        let mut children = vec![
            Button::new()
                .content(if self.active.is_some() {
                    "Finding location…"
                } else {
                    "Use current location"
                })
                .min_height(40.0)
                .is_enabled(input.enabled && self.active.is_none())
                .automation_id("current-location")
                .on_click(context.callback(|()| Message::Start))
                .into(),
        ];
        if self.active.is_some() {
            children.push(
                Button::new()
                    .content("Cancel location request")
                    .min_height(40.0)
                    .automation_id("cancel-location")
                    .on_click(context.callback(|()| Message::Cancel))
                    .into(),
            );
        }
        children.push(TextBlock::new()
            .text("Windows location access is requested only when you choose Use current location. You can also enter coordinates manually.")
            .text_wrapping(TextWrapping::Wrap).into());
        StackPanel::new().spacing(8.0).children(children).into()
    }
}

impl Location {
    fn cancel(&mut self) {
        self.active = None;
        self.request = None;
    }

    fn finish(&mut self, generation: u64, result: io::Result<Coordinates>) {
        if self.active != Some(generation) {
            return;
        }
        self.cancel();
        self.input.completed.call((self.input.epoch, result));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn cancelled_and_superseded_results_are_not_delivered() {
        let results = Rc::new(RefCell::new(Vec::new()));
        let captured = results.clone();
        let mut location = Location {
            input: Input {
                enabled: true,
                epoch: 9,
                completed: Callback::new(move |result| captured.borrow_mut().push(result)),
            },
            generation: 1,
            active: Some(1),
            request: None,
        };
        location.cancel();
        location.finish(
            1,
            Ok(Coordinates {
                latitude: 10.0,
                longitude: 20.0,
            }),
        );
        assert!(results.borrow().is_empty());
        location.active = Some(2);
        location.finish(1, Err(io::Error::other("late permission error")));
        assert_eq!(location.active, Some(2));
        location.finish(
            2,
            Ok(Coordinates {
                latitude: 40.7128,
                longitude: -74.0060,
            }),
        );
        let results = results.borrow();
        assert_eq!(results.len(), 1);
        let (epoch, coordinates) = results.first().expect("one current result");
        assert_eq!(*epoch, 9);
        assert_eq!(
            coordinates.as_ref().expect("successful location"),
            &Coordinates {
                latitude: 40.7128,
                longitude: -74.0060
            }
        );
        assert_eq!(location.active, None);
    }
}
