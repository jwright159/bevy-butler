//! This file enables #[system] to be used as follows
//! 
//! - When attached to a free-standing function, will be registered
//! to a butler plugin as defined by its attribute args
//! - When attached to a static struct function, will be registered
//! to that struct

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::{parse::{Parse, ParseStream}, Error, Expr, ExprPath, ItemFn, Meta, Path, Token};

use crate::utils::get_crate;

pub(crate) struct SystemArgs {
    pub schedule: Option<ExprPath>,
    pub plugin: Option<ExprPath>,
    pub transforms: Vec<(Path, Expr)>,
}

impl Parse for SystemArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Self {
            schedule: None,
            plugin: None,
            transforms: Default::default(),
        };

        loop {
            if input.is_empty() {
                break;
            }
            
            let meta = input.parse::<Meta>()?;
            let name_value = meta.require_name_value()?;
            match name_value.path
                .get_ident()
                .ok_or(input.error("Expected a name-value identifier"))?
                .to_string()
                .as_str()
            {
                "schedule" => {
                    if args.schedule.is_some() {
                        return Err(input.error("\"schedule\" defined more than once"));
                    }
                    else if let Expr::Path(path) = name_value.value.clone() {
                        args.schedule = Some(path);
                    }
                    else {
                        return Err(input.error("Expected a Schedule after \"schedule\""));
                    }
                },
                "plugin" => {
                    if args.plugin.is_some() {
                        return Err(input.error("\"plugin\" defined more than once"));
                    }
                    else if let Expr::Path(path) = name_value.value.clone() {
                        args.plugin = Some(path);
                    }
                    else {
                        return Err(input.error("Expected a Plugin after \"plugin\""));
                    }
                },
                _ => {
                    // Any other attributes, assume they're transformers for the system
                    args.transforms
                        .push((name_value.path.clone(), name_value.value.clone()));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(args)
    }
}

impl SystemArgs {
    /// Returns a new SystemArgs, using `self` as the default values
    /// and `new_args` as the overriding arguments.
    pub fn splat(&self, new_args: &SystemArgs) -> SystemArgs {
        Self {
            plugin: new_args.plugin.clone().or(self.plugin.clone()),
            schedule: new_args.schedule.clone().or(self.schedule.clone()),
            transforms: [self.transforms.clone(), new_args.transforms.clone()].concat(),
        }
    }
}

impl ToTokens for SystemArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(value) = self.plugin.clone() {
            tokens.extend(quote! { plugin = #value, });
        }
        if let Some(value) = self.schedule.clone() {
            tokens.extend(quote! { schedule = #value, });
        }
        for (path, value) in &self.transforms {
            tokens.extend(quote! { #path = #value, });
        }
    }
}

/// Implementation for `#[system]` on free-standing functions
/// 
/// ```
/// # use bevy_butler_proc_macro::*;
/// # use bevy::prelude::*;
/// # #[butler_plugin]
/// # struct MyPlugin;
/// #[system(schedule = Startup, plugin = MyPlugin, run_if = || true)]
/// fn hello_world() {
///     info!("Hello, world!")
/// }
/// #
/// # fn main() {
/// #   App::new().add_plugins(MyPlugin).run();
/// # }
/// #
/// ```
pub(crate) fn system_free_standing_impl(args: SystemArgs, item: ItemFn) -> Result<proc_macro2::TokenStream, proc_macro2::TokenStream> {
    let schedule = args.schedule
        .ok_or_else(|| Error::new(Span::call_site(), "#[system] requires either a defined or inherited `schedule`").into_compile_error())?;
    let plugin = args.plugin
        .ok_or_else(|| Error::new(Span::call_site(), "#[system] requires either a defined or inherited `plugin`").into_compile_error())?;

    let bevy_butler = get_crate("bevy-butler")
        .map_err(|e| Error::new(Span::call_site(), e).to_compile_error())?;

    let sys_name = &item.sig.ident;

    let transforms = if args.transforms.is_empty() {
        None
    } else {
        let transform_iter = args.transforms
        .into_iter()
        .map(|(path, expr)| quote! { #path(#expr) });
        let mut transforms = quote! { . };
        transforms.append_separated(transform_iter, Token![.](Span::call_site()));
        Some(transforms)
    };

    let butler_func_name = format_ident!("__butler_{}", sys_name);

    Ok(quote! {
        #item

        #[#bevy_butler::__internal::linkme::distributed_slice(#bevy_butler::__internal::BUTLER_SLICE)]
        #[linkme(crate = #bevy_butler::__internal::linkme)] // I LOVE UNDOCUMENTED ATTRIBUTES!!! FUCK!!!
        #[allow(non_upper_case_globals)]
        static #butler_func_name: #bevy_butler::__internal::ButlerFunc = |registry: &mut #bevy_butler::__internal::ButlerRegistry| {
            registry.entry(std::any::TypeId::of::<#plugin>())
                .or_default()
                .push(|app| { app.add_systems( #schedule, #sys_name #transforms ); } );
        };
    }.into())
}