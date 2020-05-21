extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DeriveInput, Fields};


#[proc_macro_derive(EvgFields)]
pub fn evg_fields(input: proc_macro::TokenStream) -> proc_macro::TokenStream 
{
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);


    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;

    // Generate an expression to add fields to a vector
    let add_fields = evg_fields_impl(&input.data);

    let expanded = quote! {
        // The generated impl.
        impl evergreen_rs_types::EvgFields for #name {
            fn evg_fields_nested(&self, prefix: &str, out: &mut Vec<String>) {
                #add_fields
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}


// Generate an expression to sum up the heap size of each field.
fn evg_fields_impl(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    // Expands to an expression like
                    //
                    //     0 + self.x.heap_size() + self.y.heap_size() + self.z.heap_size()
                    //
                    // but using fully qualified function call syntax.
                    //
                    // We take some care to use the span of each `syn::Field` as
                    // the span of the corresponding `heap_size_of_children`
                    // call. This way if one of the field types does not
                    // implement `HeapSize` then the compiler's error message
                    // underlines which field it is. An example is shown in the
                    // readme of the parent directory.
                    let recurse = fields.named.iter().map(|f| {
                        let name = &f.ident;
                        let name_str = name.as_ref().unwrap().to_string();
                        quote_spanned! {f.span()=>
                            out.push(evergreen_rs_types::make_name(prefix, #name_str)) ;
                        }
                    });
                    quote! {
                        #(#recurse)*
                    }
                }
                Fields::Unnamed(ref _fields) => {
                    quote! {
                    }
                }
                Fields::Unit => {
                    // Unit structs cannot own more than 0 bytes of heap memory.
                    quote!()
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}
