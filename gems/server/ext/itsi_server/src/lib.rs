use body_proxy::itsi_body_proxy::ItsiBodyProxy;
use magnus::{
    error::Result, function, method, value::Lazy, Module, Object, RClass, RHash, RModule, Ruby,
};
use regex::{Regex, RegexSet};
use request::itsi_request::ItsiRequest;
use response::itsi_response::ItsiResponse;
use server::{itsi_server::Server, signal::reset_signal_handlers};
use tracing::*;

pub mod body_proxy;
pub mod env;
pub mod request;
pub mod response;
pub mod server;

pub static ITSI_MODULE: Lazy<RModule> = Lazy::new(|ruby| ruby.define_module("Itsi").unwrap());
pub static ITSI_SERVER: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Server", ruby.class_object())
        .unwrap()
});
pub static ITSI_REQUEST: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Request", ruby.class_object())
        .unwrap()
});

pub static ITSI_RESPONSE: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("Response", ruby.class_object())
        .unwrap()
});

pub static ITSI_BODY_PROXY: Lazy<RClass> = Lazy::new(|ruby| {
    ruby.get_inner(&ITSI_MODULE)
        .define_class("BodyProxy", ruby.class_object())
        .unwrap()
});

pub fn log_debug(msg: String) {
    debug!(msg);
}
pub fn log_info(msg: String) {
    info!(msg);
}
pub fn log_warn(msg: String) {
    warn!(msg);
}
pub fn log_error(msg: String) {
    error!(msg);
}

const ROUTES: [&str; 39] = [
    r"(?-u)^/organisations/(?<organisation_id>\d+)/users/(?<user_id>\d+)$",
    r"(?-u)^/projects/(?<project_id>\d+)/tasks/(?<task_id>\d+)$",
    r"(?-u)^/products/(?<product_id>\d+)(?:/reviews/(?<review_id>\d+))?$",
    r"(?-u)^/orders/(?<order_id>\d+)/items(?:/(?<item_id>\d+))?$",
    r"(?-u)^/posts/(?<post_id>\d+)/comments(?:/(?<comment_id>\d+))?$",
    r"(?-u)^/teams/(?<team_id>\d+)(?:/members/(?<member_id>\d+))?$",
    r"(?-u)^/categories/(?<category_id>\d+)/subcategories(?:/(?<subcategory_id>\d+))?$",
    r"(?-u)^/departments/(?<department_id>\d+)/employees/(?<employee_id>\d+)$",
    r"(?-u)^/events/(?<event_id>\d+)(?:/sessions/(?<session_id>\d+))?$",
    r"(?-u)^/invoices/(?<invoice_id>\d+)/payments(?:/(?<payment_id>\d+))?$",
    r"(?-u)^/tickets/(?<ticket_id>\d+)(?:/responses/(?<response_id>\d+))?$",
    r"(?-u)^/forums/(?<forum_id>\d+)(?:/threads/(?<thread_id>\d+))?$",
    r"(?-u)^/subscriptions/(?<subscription_id>\d+)/plans(?:/(?<plan_id>\d+))?$",
    r"(?-u)^/profiles/(?<profile_id>\d+)/settings$",
    r"(?-u)^/organizations/(?<organization_id>\d+)/billing(?:/(?<billing_id>\d+))?$",
    r"(?-u)^/vendors/(?<vendor_id>\d+)/products(?:/(?<product_id>\d+))?$",
    r"(?-u)^/courses/(?<course_id>\d+)/modules(?:/(?<module_id>\d+))?$",
    r"(?-u)^/accounts/(?<account_id>\d+)(?:/transactions/(?<transaction_id>\d+))?$",
    r"(?-u)^/warehouses/(?<warehouse_id>\d+)/inventory(?:/(?<inventory_id>\d+))?$",
    r"(?-u)^/campaigns/(?<campaign_id>\d+)/ads(?:/(?<ad_id>\d+))?$",
    r"(?-u)^/applications/(?<application_id>\d+)/stages(?:/(?<stage_id>\d+))?$",
    r"(?-u)^/notifications/(?<notification_id>\d+)$",
    r"(?-u)^/albums/(?<album_id>\d+)/photos(?:/(?<photo_id>\d+))?$",
    r"(?-u)^/news/(?<news_id>\d+)/articles(?:/(?<article_id>\d+))?$",
    r"(?-u)^/libraries/(?<library_id>\d+)/books(?:/(?<book_id>\d+))?$",
    r"(?-u)^/universities/(?<university_id>\d+)/students(?:/(?<student_id>\d+))?$",
    r"(?-u)^/banks/(?<bank_id>\d+)/branches(?:/(?<branch_id>\d+))?$",
    r"(?-u)^/vehicles/(?<vehicle_id>\d+)/services(?:/(?<service_id>\d+))?$",
    r"(?-u)^/hotels/(?<hotel_id>\d+)/rooms(?:/(?<room_id>\d+))?$",
    r"(?-u)^/doctors/(?<doctor_id>\d+)/appointments(?:/(?<appointment_id>\d+))?$",
    r"(?-u)^/gyms/(?<gym_id>\d+)/memberships(?:/(?<membership_id>\d+))?$",
    r"(?-u)^/restaurants/(?<restaurant_id>\d+)/menus(?:/(?<menu_id>\d+))?$",
    r"(?-u)^/parks/(?<park_id>\d+)/events(?:/(?<event_id>\d+))?$",
    r"(?-u)^/theaters/(?<theater_id>\d+)/shows(?:/(?<show_id>\d+))?$",
    r"(?-u)^/museums/(?<museum_id>\d+)/exhibits(?:/(?<exhibit_id>\d+))?$",
    r"(?-u)^/stadiums/(?<stadium_id>\d+)/games(?:/(?<game_id>\d+))?$",
    r"(?-u)^/schools/(?<school_id>\d+)/classes(?:/(?<class_id>\d+))?$",
    r"(?-u)^/clubs/(?<club_id>\d+)/events(?:/(?<event_id>\d+))?$",
    r"(?-u)^/festivals/(?<festival_id>\d+)/tickets(?:/(?<ticket_id>\d+))?$",
];
use std::sync::LazyLock;

static REGEX_SET: LazyLock<RegexSet> = LazyLock::new(|| RegexSet::new(ROUTES).unwrap());
static REGEXES: LazyLock<Vec<Regex>> =
    LazyLock::new(|| ROUTES.iter().map(|&r| Regex::new(r).unwrap()).collect());

fn match_route(input: String) -> Result<Option<(usize, Option<RHash>)>> {
    if let Some(index) = REGEX_SET.matches(&input).iter().next() {
        let regex = &REGEXES[index];
        if let Some(captures) = regex.captures(&input) {
            let params = RHash::with_capacity(captures.len());
            for name in regex.capture_names().flatten() {
                if let Some(value) = captures.name(name) {
                    params.aset(name, value.as_str()).ok();
                }
            }
            return Ok(Some((index, Some(params))));
        }
        return Ok(Some((index, None)));
    }
    Ok(None)
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<()> {
    itsi_tracing::init();
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let itsi = ruby.get_inner(&ITSI_MODULE);
    itsi.define_singleton_method("match_route", function!(match_route, 1))?;
    itsi.define_singleton_method("log_debug", function!(log_debug, 1))?;
    itsi.define_singleton_method("log_info", function!(log_info, 1))?;
    itsi.define_singleton_method("log_warn", function!(log_warn, 1))?;
    itsi.define_singleton_method("log_error", function!(log_error, 1))?;

    let server = ruby.get_inner(&ITSI_SERVER);
    server.define_singleton_method("new", function!(Server::new, -1))?;
    server.define_singleton_method("reset_signal_handlers", function!(reset_signal_handlers, 0))?;
    server.define_method("start", method!(Server::start, 0))?;
    server.define_method("stop", method!(Server::stop, 0))?;

    let request = ruby.get_inner(&ITSI_REQUEST);
    request.define_method("path", method!(ItsiRequest::path, 0))?;
    request.define_method("script_name", method!(ItsiRequest::script_name, 0))?;
    request.define_method("query_string", method!(ItsiRequest::query_string, 0))?;
    request.define_method("method", method!(ItsiRequest::method, 0))?;
    request.define_method("version", method!(ItsiRequest::version, 0))?;
    request.define_method("rack_protocol", method!(ItsiRequest::rack_protocol, 0))?;
    request.define_method("host", method!(ItsiRequest::host, 0))?;
    request.define_method("headers", method!(ItsiRequest::headers, 0))?;
    request.define_method("scheme", method!(ItsiRequest::scheme, 0))?;
    request.define_method("remote_addr", method!(ItsiRequest::remote_addr, 0))?;
    request.define_method("port", method!(ItsiRequest::port, 0))?;
    request.define_method("body", method!(ItsiRequest::body, 0))?;
    request.define_method("response", method!(ItsiRequest::response, 0))?;
    request.define_method("json?", method!(ItsiRequest::is_json, 0))?;
    request.define_method("html?", method!(ItsiRequest::is_html, 0))?;

    let body_proxy = ruby.get_inner(&ITSI_BODY_PROXY);
    body_proxy.define_method("gets", method!(ItsiBodyProxy::gets, 0))?;
    body_proxy.define_method("each", method!(ItsiBodyProxy::each, 0))?;
    body_proxy.define_method("read", method!(ItsiBodyProxy::read, -1))?;
    body_proxy.define_method("close", method!(ItsiBodyProxy::close, 0))?;

    let response = ruby.get_inner(&ITSI_RESPONSE);
    response.define_method("add_header", method!(ItsiResponse::add_header, 2))?;
    response.define_method("status=", method!(ItsiResponse::set_status, 1))?;
    response.define_method("send_frame", method!(ItsiResponse::send_frame, 1))?;
    response.define_method("send_and_close", method!(ItsiResponse::send_and_close, 1))?;
    response.define_method("close_write", method!(ItsiResponse::close_write, 0))?;
    response.define_method("close_read", method!(ItsiResponse::close_read, 0))?;
    response.define_method("close", method!(ItsiResponse::close, 0))?;
    response.define_method("hijack", method!(ItsiResponse::hijack, 1))?;
    response.define_method("json?", method!(ItsiResponse::is_json, 0))?;
    response.define_method("html?", method!(ItsiResponse::is_html, 0))?;

    Ok(())
}
