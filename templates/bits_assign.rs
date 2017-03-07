{{#each arguments as |argument| ~}}
{{snake argument.name}} : placeholder_tuple.{{argument.id}},
{{/each ~}}
