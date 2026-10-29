import React, { memo, useCallback } from "react";
import { MoonIcon, SunIcon } from "@storybook/icons";
import { ToggleButton } from "storybook/internal/components";
import { addons, types, useGlobals } from "storybook/manager-api";

const ADDON_ID = "ui-color-mode-toggle";
const TOOL_ID = `${ADDON_ID}/tool`;

const ColorModeToggle = memo(function ColorModeToggle() {
  const [globals, updateGlobals] = useGlobals();
  const isDark = globals.colorMode === "dark";

  const toggle = useCallback(() => {
    updateGlobals({ colorMode: isDark ? "light" : "dark" });
  }, [isDark, updateGlobals]);

  const label = isDark ? "Switch to light mode" : "Switch to dark mode";

  return (
    <ToggleButton
      key={TOOL_ID}
      size="small"
      variant="ghost"
      padding="small"
      pressed={isDark}
      title={label}
      ariaLabel={label}
      onClick={toggle}
    >
      {isDark ? <MoonIcon /> : <SunIcon />}
    </ToggleButton>
  );
});

addons.register(ADDON_ID, () => {
  addons.add(TOOL_ID, {
    type: types.TOOL,
    title: "Color mode",
    match: ({ viewMode }) => !!(viewMode && viewMode.match(/^(story|docs)$/)),
    render: () => <ColorModeToggle />,
  });
});
