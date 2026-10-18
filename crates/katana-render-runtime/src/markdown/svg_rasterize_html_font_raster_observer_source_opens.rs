const SHA256_BYTES: usize = 32;

#[derive(Clone, Copy)]
struct Open {
    key: [u8; SHA256_BYTES],
    frame: u64,
    attempt: u64,
}

#[derive(Clone, Default)]
pub(super) struct OpenTracker {
    slots: [Option<Open>; 2],
    next: usize,
    pub(super) repeated: u64,
    pub(super) overflow: bool,
}

impl OpenTracker {
    pub(super) fn observe(&mut self, key: [u8; SHA256_BYTES], frame: u64, attempt: u64) {
        if frame == 0 || attempt == 0 {
            return;
        }
        let previous = self.slots.iter().flatten().find(|open| open.key == key);
        if frame != 0
            && attempt != 0
            && previous.is_some_and(|open| open.frame == frame && open.attempt != attempt)
        {
            match self.repeated.checked_add(1) {
                Some(next) => self.repeated = next,
                None => self.overflow = true,
            }
        }
        self.update_slot(key, frame, attempt);
    }

    fn update_slot(&mut self, key: [u8; SHA256_BYTES], frame: u64, attempt: u64) {
        if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|open| open.key == key))
        {
            *slot = Some(Open {
                key,
                frame,
                attempt,
            });
        } else {
            self.slots[self.next] = Some(Open {
                key,
                frame,
                attempt,
            });
            self.next = (self.next + 1) % self.slots.len();
        }
    }
}
