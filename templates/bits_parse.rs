 placeholder_tuple : bits!(
    do_parse!(
      take_bits!(u8, 2) >>
      {{#each arguments as |argument| ~}}
      {{snake argument.name}}: map!(take_bits!(u8, 1), |i| i != 0) >>
      {{/each ~}}
      (
        {{#each arguments as |argument| ~}}
        {{snake argument.name}},
        {{/each ~}}
        0
      )
    )
  ) >>
