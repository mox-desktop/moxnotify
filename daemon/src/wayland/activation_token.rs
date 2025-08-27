use crate::Moxnotify;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: <xdg_activation_token_v1::XdgActivationTokenV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event
            && let Some(surface) = state.surface.as_mut()
        {
            surface.token = Some(token.into());
        }
    }
}

delegate_noop!(Moxnotify: xdg_activation_v1::XdgActivationV1);
