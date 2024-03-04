use crate::router::Route;
use crate::switch::switch;

use yew::prelude::*;
use yew_router::prelude::{BrowserRouter, Switch};

#[function_component(App)]
pub fn app() -> Html {
    use crate::components::theme::theme_ctx::WithTheme;
    use crate::components::theme::toggle::ThemeToggle;

    html! {
        <WithTheme>
            <ThemeToggle/>
            <BrowserRouter>
                <Switch<Route> render={switch} />
            </BrowserRouter>
        </WithTheme>
    }
}
