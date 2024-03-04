mod app;
mod components;
mod router;
mod switch;
mod ws;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
