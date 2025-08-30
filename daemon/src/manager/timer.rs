pub struct NotificationTimer {
    loop_handle: LoopHandle<'static, Moxnotify>,
    registration_token: RegistrationToken,
}

pub fn new(loop_handle: LoopHandle<'static, Moxnotify>) -> Self {
    Self {}
}
