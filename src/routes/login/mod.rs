const FLASH_COOKIE_NAME: &str = "flash";
const FLASH_COOKIE_PATH: &str = "/login";

mod get;
pub use get::login_form;

mod post;
pub use post::login;
