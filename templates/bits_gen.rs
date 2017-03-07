>> gen_be_u8!(
  make_bit_field(
    (vec![
      {{#each arguments as |argument| ~}}
      method.{{snake argument.name}}.clone(),
      {{/each ~}}]
    ).as_slice()
  )
)
