import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useAppearancePreferences } from "./useAppearancePreferences";

describe("appearance preferences", () => {
  afterEach(cleanup);

  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("defaults to simple view and the unset system theme", () => {
    const { result } = renderHook(() => useAppearancePreferences());

    expect(result.current.simpleMode).toBe(true);
    expect(result.current.theme).toBe("system");
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });

  it("restores stored view and theme preferences", () => {
    window.localStorage.setItem("ctx.simple-mode", "detailed");
    window.localStorage.setItem("ctx.theme", "dark");

    const { result } = renderHook(() => useAppearancePreferences());

    expect(result.current.simpleMode).toBe(false);
    expect(result.current.theme).toBe("dark");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  });

  it("persists explicit choices and unsets the system theme", () => {
    const { result } = renderHook(() => useAppearancePreferences());

    act(() => {
      result.current.setSimpleMode(false);
      result.current.setTheme("light");
    });

    expect(window.localStorage.getItem("ctx.simple-mode")).toBe("detailed");
    expect(window.localStorage.getItem("ctx.theme")).toBe("light");
    expect(document.documentElement).toHaveAttribute("data-theme", "light");

    act(() => result.current.setTheme("system"));

    expect(window.localStorage.getItem("ctx.theme")).toBeNull();
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });
});
