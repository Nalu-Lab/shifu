import { describe, expect, it } from "vitest";
import { projectControl, submitAction } from "./newpopover";

describe("new-task form with no recent folders", () => {
  it("offers a plain choose-folder button, since a lone preselected dropdown entry never fires change", () => {
    expect(projectControl([])).toBe("button");
    expect(projectControl(["/Users/me/code/app"])).toBe("dropdown");
  });

  it("opens the folder dialog from 建立 instead of refusing", () => {
    expect(submitAction(null)).toBe("choose");
    expect(submitAction("/Users/me/code/app")).toBe("create");
  });
});
