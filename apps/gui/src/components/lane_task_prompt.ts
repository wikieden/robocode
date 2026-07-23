import { translate, type Locale } from "../i18n/catalog";
import "./lane_task_prompt.css";

export function renderLaneTaskPrompt(
  host: HTMLElement,
  locale: Locale,
  title: string,
  onSubmit: (task: string) => void,
  onCancel: () => void,
  initialTask = "",
  onTaskChange: (task: string) => void = () => undefined,
): HTMLElement {
  const backdrop = document.createElement("div");
  backdrop.className = "lane-task-backdrop";
  const dialog = document.createElement("section");
  dialog.className = "lane-task-prompt";
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-label", title);
  const heading = document.createElement("h2");
  heading.textContent = title;
  const input = document.createElement("textarea");
  input.dataset.laneTask = "true";
  input.rows = 4;
  input.value = initialTask;
  input.placeholder = translate(locale, "d1.task.placeholder", {});
  input.setAttribute("aria-label", translate(locale, "d1.task.label", {}));
  const actions = document.createElement("div");
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.textContent = translate(locale, "d1.task.cancel", {});
  const submit = document.createElement("button");
  submit.type = "button";
  submit.dataset.laneTaskSubmit = "true";
  submit.textContent = translate(locale, "d1.task.submit", {});
  submit.disabled = !input.value.trim();
  let composing = false;
  const finish = (): void => {
    const task = input.value.trim();
    if (!task || composing) return;
    onSubmit(task);
    backdrop.remove();
  };
  input.addEventListener("compositionstart", () => (composing = true));
  input.addEventListener("compositionend", () => {
    composing = false;
    submit.disabled = !input.value.trim();
  });
  input.addEventListener("input", () => {
    onTaskChange(input.value);
    submit.disabled = !input.value.trim();
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      backdrop.remove();
    } else if (event.key === "Enter" && !event.shiftKey && !composing) {
      event.preventDefault();
      finish();
    }
  });
  cancel.addEventListener("click", () => {
    onCancel();
    backdrop.remove();
  });
  submit.addEventListener("click", finish);
  actions.append(cancel, submit);
  dialog.append(heading, input, actions);
  backdrop.append(dialog);
  host.append(backdrop);
  input.focus();
  return backdrop;
}
