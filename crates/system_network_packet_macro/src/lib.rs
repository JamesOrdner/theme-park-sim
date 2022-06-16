extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(NetworkPacketTypes)]
pub fn derive_network_packet_enum(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let data = match data {
        Data::Enum(data) => data,
        _ => panic!("NetworkPacketEnum may only be used on enums"),
    };

    let variant_idents = data.variants.iter().map(|variant| variant.ident.clone());
    let variant_ref_idents = data
        .variants
        .iter()
        .map(|variant| format_ident!("{}Ref", variant.ident));

    let variant_idents_2 = variant_idents.clone();
    let variant_ref_idents_2 = variant_ref_idents.clone();

    quote!(
        pub enum PacketRef<'a> {
            #(#variant_idents(#variant_ref_idents<'a>),) *
        }

        impl<'a> From<&'a [u8]> for PacketRef<'a> {
            fn from(data: &'a [u8]) -> Self {
                match data[0] {
                    #(a if a == #ident::#variant_idents_2 as u8 => {
                        Self::#variant_idents_2(#variant_ref_idents_2(data[1..].try_into().unwrap()))
                    },) *
                    _ => unreachable!(),
                }
            }
        }
    )
    .into()
}

#[proc_macro_derive(NetworkPacket)]
pub fn derive_network_packet(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let data = match data {
        Data::Struct(data) => data,
        _ => panic!("NetworkPacket may only be used on structs"),
    };

    let field_idents = data.fields.iter().map(|field| field.ident.clone().unwrap());
    let field_types = data.fields.iter().map(|field| field.ty.clone());
    let num_fields = data.fields.iter().count();

    let field_idents_2 = field_idents.clone();
    let field_types_2 = field_types.clone();
    let field_types_3 = field_types.clone();
    let field_types_4 = field_types.clone();

    let field_indices = field_types.clone().enumerate().map(|(i, _)| i);

    let field_sizes_ident = format_ident!("{}_FIELD_SIZES", ident);
    let packet_size_ident = format_ident!("{}_PACKET_SIZE", ident);
    let ref_ident = format_ident!("{}Ref", ident);

    quote!(
        #[allow(non_upper_case_globals)]
        const #packet_size_ident: usize = #(std::mem::size_of::<#field_types_3>() +)* 1;

        #[allow(non_upper_case_globals)]
        const #field_sizes_ident: [usize; #num_fields] = [#(std::mem::size_of::<#field_types_4>(),)*];

        impl #ident {
            pub fn serialize(&self) -> [u8; #packet_size_ident ] {
                let mut data = [0; #packet_size_ident];
                data[0] = PacketType::#ident as u8;

                let mut i = 1;
                #(
                    let size = std::mem::size_of::<#field_types>();
                    data[i..i + size].copy_from_slice(&self.#field_idents.to_le_bytes());
                    i += size;
                ) *

                data
            }
        }

        pub struct #ref_ident<'a>(&'a [u8; #packet_size_ident - 1]);

        impl #ref_ident<'_> {
            #(
                pub fn #field_idents_2(&self) -> #field_types_2 {
                    let offset = (0..#field_indices).map(|i| #field_sizes_ident[i]).sum();
                    let size = std::mem::size_of::<#field_types_2>();
                    #field_types_2::from_le_bytes(self.0[offset..offset + size].try_into().unwrap())
                }
            ) *
        }
    )
    .into()
}
