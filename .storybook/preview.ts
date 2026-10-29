import type { Preview } from "@storybook/svelte-vite";
import { withThemeByDataAttribute } from "@storybook/addon-themes";
import { syncCatalogStoryLayout } from "@lapismd/design-core/storybook/catalog-layout";
import "@lapismd/design-core/storybook.css";
import "@lapismd/design-core/themes/lapis.css";

const preview: Preview = {
  globalTypes: {
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
        document.documentElement.classList.toggle(
          "dark",
          context.globals.colorMode === "dark",
        );
      }
      return story();
    },
  ],
  parameters: {
    layout: "fullscreen",
    backgrounds: { disable: true },
    themes: { disable: true },
    docs: { toc: true },
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
