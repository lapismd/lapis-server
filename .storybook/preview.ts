import "./docs.css";
import type { Preview } from "@storybook/svelte-vite";
import { withThemeByDataAttribute } from "@storybook/addon-themes";
import { syncCatalogStoryLayout } from "@lapismd/design-core/storybook/catalog-layout";
import "@lapismd/design-core/storybook.css";
import "@lapismd/design-core/themes/lapis.css";
import "@lapis-notes/ui/theme.css";
import "@lapis-notes/ui/codemirror-autocomplete.css";
import "../web/src/styles/host.css";
import "../web/src/styles/login.css";
import "../web/src/styles/vault-launcher.css";
import "../web/src/styles/vault-dialogs.css";
import { requirementTocLabel } from "./requirement-toc";

const preview: Preview = {
  globalTypes: {
    colorMode: {
      description: "Appearance",
      toolbar: {
        icon: "mirror",
        items: [
          { value: "light", title: "Light" },
          { value: "dark", title: "Dark" },
        ],
        dynamicTitle: true,
      },
    },
    theme: {
      description: "Brand theme",
      toolbar: {
        icon: "paintbrush",
        items: [
          { value: "lapis", title: "Lapis" },
          { value: "default", title: "Default" },
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: {
    theme: "lapis",
    colorMode: "light",
  },
  decorators: [
    withThemeByDataAttribute({
      themes: { lapis: "lapis", default: "default" },
      defaultTheme: "lapis",
      attributeName: "data-ui-theme",
    }),
    (story, context) => {
      if (typeof document !== "undefined") {
        syncCatalogStoryLayout(document, context);
        const dark = context.globals.colorMode === "dark";
        for (const name of ["dark", "theme-dark"])
          document.documentElement.classList.toggle(name, dark);
        for (const name of ["light", "theme-light"])
          document.documentElement.classList.toggle(name, !dark);
      }
      return story();
    },
  ],
  parameters: {
    layout: "fullscreen",
    a11y: { test: "error" },
    backgrounds: { disable: true },
    themes: { disable: true },
    // Requirement ids come from h2 text, the same headings design-core selects.
    // h3 is only "Acceptance details", so it stays out of the right-hand nav.
    docs: {
      toc: {
        headingSelector: "h2",
        title: "On this page",
        unsafeTocbotOptions: {
          headingLabelCallback: requirementTocLabel,
        },
      },
    },
    options: {
      storySort: {
        order: [
          "Specification",
          [
            "Current specification",
            "Hosted server",
            "Hosted contract",
            "Static delivery",
            "Governance",
            "Backlog",
            "Native application services",
            "Hosted qualification and delivery",
            "Verification",
          ],
          "Shell",
          "*",
        ],
      },
    },
  },
};

export default preview;
