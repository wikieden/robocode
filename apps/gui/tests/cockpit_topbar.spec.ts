// @vitest-environment jsdom

// The cockpit titlebar's git block renders host-projected workspace facts and
// nothing else. It is read-only: frontend-contract-v1 carries no operator git
// command, so the only interactive element is the worktrees chip, which
// navigates to the Lane monitor.
import { describe, expect, test, vi } from "vitest";

import { renderCockpitTopbar } from "../src/components/cockpit_topbar";
import type { D1CockpitProjection, TopbarSourceProjection } from "../src/models/workspace";
import { D1_PROJECTION } from "./support/d1_projection";

const SOURCE: TopbarSourceProjection = {
  project: "viden",
  branch: "codex/v3-gui-client",
  ahead: 2,
  behind: 1,
  dirty: false,
  status: "ready",
  truncated: false,
  laneWorktreeCount: 4,
};

function projection(
  topbarSource: TopbarSourceProjection | null = SOURCE,
): D1CockpitProjection {
  return { ...structuredClone(D1_PROJECTION), topbarSource };
}

function gitops(element: HTMLElement): HTMLElement | null {
  return element.querySelector<HTMLElement>("[data-topbar-gitops]");
}

describe("cockpit titlebar git block", () => {
  test("the project selector names the project and its branch", () => {
    const { element } = renderCockpitTopbar(projection(), "en", false);
    const projsel = element.querySelector<HTMLElement>(".projsel");

    expect(projsel?.textContent).toContain("viden");
    expect(projsel?.querySelector(".br")?.textContent).toContain("codex/v3-gui-client");
  });

  test("without a Core-published project name the workspace path stays the label", () => {
    const base = projection();
    const { element } = renderCockpitTopbar(
      { ...base, topbarSource: { ...SOURCE, project: null } },
      "en",
      false,
    );
    // The path Core published, never a name derived from it.
    expect(element.querySelector(".projsel")?.textContent).toContain("/workspace/viden");
  });

  test("without a picker handler the selector stays an inert label", () => {
    const { element } = renderCockpitTopbar(projection(), "en", false);
    const projsel = element.querySelector<HTMLElement>(".projsel")!;

    // The chevron promises a popover. Without a bound handler there is nothing
    // to open, so it stays out rather than being enabled and inert.
    expect(projsel.tagName).toBe("SPAN");
    expect(projsel.textContent).not.toContain("▾");
    expect(projsel.getAttribute("role")).toBeNull();
    expect(projsel.querySelector("button")).toBeNull();
  });

  test("a bound picker handler turns the selector into the design's ▾ button", () => {
    const onOpenProjectPicker = vi.fn();
    const { element } = renderCockpitTopbar(projection(), "en", false, undefined, {
      onOpenProjectPicker,
    });
    const projsel = element.querySelector<HTMLButtonElement>(".projsel")!;

    expect(projsel.tagName).toBe("BUTTON");
    expect(projsel.dataset.projectSelector).toBe("true");
    expect(projsel.getAttribute("aria-haspopup")).toBe("dialog");
    expect(projsel.getAttribute("aria-expanded")).toBe("false");
    expect(projsel.querySelector(".chev")?.textContent).toBe("▾");
    // A drag region swallows clicks, so the button must not claim one.
    expect(projsel.dataset.tauriDragRegion).toBeUndefined();
    projsel.click();
    expect(onOpenProjectPicker).toHaveBeenCalledTimes(1);
  });

  test("the no-project welcome keeps the selector inert even with a handler", () => {
    const { element } = renderCockpitTopbar(projection(), "en", true, undefined, {
      onOpenProjectPicker: vi.fn(),
    });
    const projsel = element.querySelector<HTMLElement>(".projsel")!;

    // There is no workspace to switch away from yet.
    expect(projsel.tagName).toBe("SPAN");
    expect(projsel.textContent).not.toContain("▾");
  });

  test("an open picker is announced on the selector", () => {
    const { element } = renderCockpitTopbar(projection(), "en", false, undefined, {
      onOpenProjectPicker: vi.fn(),
      projectPickerOpen: true,
    });
    expect(element.querySelector(".projsel")?.getAttribute("aria-expanded")).toBe("true");
  });

  test("the sync chip states ahead and behind as status, not as an action", () => {
    const { element } = renderCockpitTopbar(projection(), "en", false);
    const sync = element.querySelector<HTMLElement>("[data-topbar-sync]")!;

    expect(sync.querySelector(".up")?.textContent).toBe("↑2");
    expect(sync.querySelector(".down")?.textContent).toBe("↓1");
    expect(sync.tagName).toBe("SPAN");
    expect(sync.getAttribute("role")).toBe("status");
    expect(sync.title).toContain("GUI-CORE-020");
  });

  test("a dirty workspace is marked next to the branch and a clean one is not", () => {
    const dirty = renderCockpitTopbar(
      { ...projection(), topbarSource: { ...SOURCE, dirty: true } },
      "en",
      false,
    );
    const marker = dirty.element.querySelector<HTMLElement>("[data-topbar-dirty]");
    expect(marker).not.toBeNull();
    expect(marker?.title).toContain("Uncommitted");

    const clean = renderCockpitTopbar(projection(), "en", false);
    expect(clean.element.querySelector("[data-topbar-dirty]")).toBeNull();
  });

  test("a truncated sample renders its facts behind a truncation marker", () => {
    const { element } = renderCockpitTopbar(
      { ...projection(), topbarSource: { ...SOURCE, status: "truncated", truncated: true } },
      "en",
      false,
    );
    const marker = element.querySelector<HTMLElement>("[data-topbar-truncated]");
    expect(marker).not.toBeNull();
    expect(marker?.title).toContain("partial");
    // The published counts still render; only their completeness is qualified.
    expect(element.querySelector("[data-topbar-sync] .up")?.textContent).toBe("↑2");
  });

  test("the git block is omitted entirely when Core published no usable source", () => {
    const { element } = renderCockpitTopbar(projection(null), "en", false);

    expect(gitops(element)).toBeNull();
    expect(element.querySelector("[data-topbar-sync]")).toBeNull();
    expect(element.querySelector("[data-topbar-worktrees]")).toBeNull();
    // The project label survives; only the git facts disappear.
    expect(element.querySelector(".projsel")?.textContent).toContain("/workspace/viden");
  });

  test("the git block is omitted on the welcome screen, where no project is open", () => {
    const { element } = renderCockpitTopbar(projection(), "en", true);
    expect(gitops(element)).toBeNull();
  });

  test("the worktrees chip counts Core's Lane worktrees and opens the Lane monitor", () => {
    const onNavigate = vi.fn();
    const { element } = renderCockpitTopbar(projection(), "en", false, onNavigate);
    const chip = element.querySelector<HTMLButtonElement>("[data-topbar-worktrees]")!;

    expect(chip.textContent).toContain("4 worktrees");
    expect(chip.disabled).toBe(false);
    chip.click();
    expect(onNavigate).toHaveBeenCalledExactlyOnceWith("d10");
  });

  test("a single worktree reads in the singular", () => {
    const { element } = renderCockpitTopbar(
      { ...projection(), topbarSource: { ...SOURCE, laneWorktreeCount: 1 } },
      "en",
      false,
    );
    expect(element.querySelector("[data-topbar-worktrees]")?.textContent).toContain("1 worktree");
    expect(element.querySelector("[data-topbar-worktrees]")?.textContent).not.toContain(
      "worktrees",
    );
  });

  test("without a navigation handler the worktrees chip is visible but disabled", () => {
    const { element } = renderCockpitTopbar(projection(), "en", false);
    const chip = element.querySelector<HTMLButtonElement>("[data-topbar-worktrees]")!;
    expect(chip.disabled).toBe(true);
  });

  test("the worktrees chip is the git block's only control", () => {
    const { element } = renderCockpitTopbar(projection(), "en", false, vi.fn());
    expect(gitops(element)!.querySelectorAll("button")).toHaveLength(1);
  });

  test("the context-drawer toggle keeps its own identity beside the git block", () => {
    const topbar = renderCockpitTopbar(projection(), "en", false, vi.fn());
    expect(topbar.contextDrawerToggle.dataset.contextDrawerToggle).toBe("true");
    expect(topbar.contextDrawerToggle.getAttribute("aria-controls")).toBe("d1-context-dock");
    expect(gitops(topbar.element)!.contains(topbar.contextDrawerToggle)).toBe(false);
  });
});
