use std::collections::HashMap;

use convert_case::{Case, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream, Parser, Result};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

struct Args {
    pub actor_type: Type,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Args {
            actor_type: input.parse()?,
        })
    }
}

#[proc_macro_attribute]
pub fn crdt(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    let args = parse_macro_input!(args as Args);

    let v_clock_type = args.actor_type;

    if let syn::Data::Struct(ref mut struct_data) = &mut ast.data {
        if let syn::Fields::Named(fields) = &mut struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { v_clock: crdts::VClock<#v_clock_type> })
                    .unwrap(),
            );
        }
    }

    quote! {
        #[derive(crdts_derive::CRDT)]
        #ast
    }
    .into()
}

#[proc_macro_derive(CRDT)]
pub fn cmrdt_macro_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse(input).unwrap();
    let expanded = impl_cmrdt_macro(input);
    proc_macro::TokenStream::from(expanded)
}

fn impl_cmrdt_macro(input: syn::DeriveInput) -> TokenStream {
    let name = &input.ident;
    let data = &input.data;

    let fields = list_fields(data);

    let error_name = Ident::new(&(name.to_string() + "CrdtError"), Span::call_site());
    let error_enum = build_error(&fields);

    let op_name = Ident::new(&(name.to_string() + "CrdtOp"), Span::call_site());
    let op_param = build_op(&fields);

    let impl_apply = impl_apply(&fields);
    let impl_validate = impl_validate(&fields);

    quote! {
        #[derive(std::fmt::Debug, PartialEq, Eq)]
        pub enum #error_name {
            NoneOp,
            #error_enum
        }

        impl std::fmt::Display for #error_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self, f)
            }
        }

        impl std::error::Error for #error_name {}

        #[allow(clippy::type_complexity)]
        pub struct #op_name {
            #op_param
        }

        impl CmRDT for #name {
            type Op = #op_name;
            type Validation = #error_name;

            fn apply(&mut self, op: Self::Op) {
                #impl_apply
            }

            fn validate_op(&self, op: &Self::Op) -> Result<(), Self::Validation> {
                #impl_validate
            }
        }
    }
}

fn list_fields(data: &Data) -> HashMap<String, Type> {
    let mut field_list = HashMap::new();
    if let Data::Struct(ref data) = *data {
        if let Fields::Named(ref fields) = data.fields {
            for f in &fields.named {
                field_list.insert(f.ident.clone().unwrap().to_string(), f.ty.clone());
            }
        }
    }
    field_list
}

fn build_error(fields: &HashMap<String, Type>) -> TokenStream {
    let recurse = fields.iter().map(|f| {
        let pascal_name = f.0.to_case(Case::Pascal);
        let name = Ident::new(&pascal_name, Span::call_site());
        let ty = f.1;
        quote_spanned! { Span::call_site() =>
            #name(<#ty as CmRDT>::Validation),
        }
    });
    quote! {
        #(#recurse)*
    }
}

fn build_op(fields: &HashMap<String, Type>) -> TokenStream {
    let mut spans = vec![];
    for f in fields {
        let mut name = f.0.to_owned();
        let ty = &f.1;
        if name == "v_clock" {
            name = "dot".to_owned();
            let name = Ident::new(&name, Span::call_site());
            spans.push(quote_spanned! { Span::call_site() =>
                #name: <#ty as CmRDT>::Op,
            });
        } else {
            name += "_op";
            let name = Ident::new(&name, Span::call_site());
            spans.push(quote_spanned! { Span::call_site() =>
                #name: Option<<#ty as CmRDT>::Op>,
            });
        }
    }
    quote! {
        #(#spans)*
    }
}

fn impl_apply(fields: &HashMap<String, Type>) -> TokenStream {
    let op_params = op_params(fields);
    let nones = count_none(fields);

    let apply = fields.keys().map(|f| {
        if f == "v_clock" {
            return quote_spanned! { Span::call_site() => };
        }
        let field = Ident::new(f, Span::call_site());
        let op = Ident::new(&(f.to_owned() + "_op"), Span::call_site());
        quote_spanned! { Span::call_site() =>
            if let Some(#op) = #op {
                self.#field.apply(#op);
            }
        }
    });

    let apply = quote! {
        #(#apply)*
    };

    quote! {
        let Self::Op {
            dot,
            #op_params
        } = op;
        if self.v_clock.get(&dot.actor) >= dot.counter {
            return;
        }
        match (#op_params) {
            (#nones) => return,
            (#op_params) => {
                #apply
            }
        }
        self.v_clock.apply(dot);
    }
}

fn impl_validate(fields: &HashMap<String, Type>) -> TokenStream {
    let op_params = op_params(fields);
    let nones = count_none(fields);

    let validate = fields.keys().map(|f| {
        if f == "v_clock" {
            return quote_spanned! {Span::call_site()=>};
        }
        let pascal_name = f.to_case(Case::Pascal);
        let error_name = Ident::new(&pascal_name, Span::call_site());
        let field = Ident::new(f, Span::call_site());
        let op = Ident::new(&(f.to_owned() + "_op"), Span::call_site());
        quote_spanned! { Span::call_site() =>
            if let Some(#op) = #op {
                self.#field.validate_op(#op).map_err(Self::Validation::#error_name)?;
            }
        }
    });

    let validate = quote! {
        #(#validate)*
    };

    quote! {
        let Self::Op {
            dot,
            #op_params
        } = op;
        self.v_clock
            .validate_op(dot)
            .map_err(Self::Validation::VClock)?;
        match (#op_params) {
            (#nones) => return Err(Self::Validation::NoneOp),
            (#op_params) => {
                #validate
                return Ok(());
            }
        }
    }
}

fn count_none(fields: &HashMap<String, Type>) -> TokenStream {
    let nones = fields.keys().filter_map(|f| {
        if f != "v_clock" {
            Some(Ident::new("None", Span::call_site()))
        } else {
            None
        }
    });
    let nones = quote! {
        #(#nones,)*
    };
    nones
}

fn op_params(fields: &HashMap<String, Type>) -> TokenStream {
    let op_params = fields.keys().filter_map(|f| {
        if f != "v_clock" {
            Some(Ident::new(&(f.to_owned() + "_op"), Span::call_site()))
        } else {
            None
        }
    });
    let op_params = quote! {
        #(#op_params,)*
    };
    op_params
}
