/** @type {import("prettier").Config} */
const config = {
    useTabs: false,
    singleQuote: false,
    trailingComma: "none",
    printWidth: 100,
    plugins: ["prettier-plugin-svelte", "prettier-plugin-tailwindcss"],
    overrides: [{ files: "*.svelte", options: { parser: "svelte" } }],
    tailwindStylesheet: "./src/routes/layout.css",
    semi: false,
    tabWidth: 4
}

export default config
