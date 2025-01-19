use proc_macro::TokenStream as TokenStream1;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use structs::ConfigSystemsInput;
use syn::{parse::{Parse, Parser}, Attribute, Item, MetaList};

mod structs;

pub(crate) const CONFIG_SYSTEMS_DEFAULT_ARGS_IDENT: &'static str = "_butler_config_systems_defaults";

pub(crate) fn macro_impl(body: TokenStream1) -> syn::Result<TokenStream2> {
    // Parse the arguments
    let input = ConfigSystemsInput::parse.parse(body)?;
    let defaults = input.system_args;
    let mut items = input.items;

    let arg_metas = defaults.get_metas();
    let config_attr: Attribute = Attribute {
        pound_token: Default::default(),
        style: syn::AttrStyle::Outer,
        bracket_token: Default::default(),
        meta: syn::Meta::List(MetaList {
            path: syn::parse_str(&CONFIG_SYSTEMS_DEFAULT_ARGS_IDENT)?,
            delimiter: syn::MacroDelimiter::Brace(Default::default()),
            tokens: arg_metas.to_token_stream(),
        })
    };

    // For every item, insert the dummy attribute `_butler_config_systems_default`
    // Systems will read from this attribute and apply the default arguments to their
    // own arguments.
    //
    // Any non-systems will simply ignore the attribute.
    for item in items.iter_mut() {
        match item {
            // Could be a system with `#[system]`
            Item::Fn(item_fn) => {
                item_fn.attrs.push(config_attr.clone());
            }
            // Could be `config_systems!`
            Item::Macro(_item_macro) => todo!(),
            _ => (),
        }
    }

    // Unwrap the items
    Ok(quote! {
        #(#items)*
    })
}