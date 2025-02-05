# Project Info

This is a learning project where i learn web development and/with rust

I also vaguely try to follow [this HTMX course](https://www.youtube.com/watch?v=x7v6SNIgJpE) from primeagen

I chose this Techstack:

- [axum](https://github.com/tokio-rs/axum)
- [minijinja](https://github.com/mitsuhiko/minijinja)
- [htmx](https://github.com/bigskysoftware/htmx)
- [tailwindcss](https://github.com/tailwindlabs/tailwindcss)

## Goals

- use as few javascript as possible
- become a fullstack dev

## setting up the project

```bash
npm install
npx @tailwindcss/cli -i styles/tailwind.css -o assets/main.css
cargo build
```

## developing

```bash
# for tailwind to update main.css when editing css in your html files
npx @tailwindcss/cli -i styles/tailwind.css -o assets/main.css --watch
```

```bash
# automatically recompiles when file in project is saved
cargo watch -x run
```

(maybe can make automatic browser reload work but didnt bother looking yet)

## Thoughts

For learning, something more simple/barebones couldve been better.
Im noticing quite often, that i dont really understand rusts basics yet or even how the async stuff works.
But the things i do understand, feel great and i learned very much already.
So i dont care if this is too much to get into webdev because its a project that motivates me to get keep going.
