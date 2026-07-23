import { translate, type Locale } from "../i18n/catalog";

export interface ProviderHealthView {
  providerId: string;
  model: string;
  status: string;
  warning: boolean;
}

export function renderProviderHealth(
  provider: ProviderHealthView | null,
  locale: Locale,
): HTMLElement {
  const container = document.createElement("aside");
  container.className = "provider-health";
  if (!provider) {
    container.hidden = true;
    return container;
  }

  container.dataset.providerHealth = provider.status;
  if (provider.warning) {
    container.dataset.providerWarning = "true";
  }
  container.textContent = provider.warning
    ? translate(locale, "d11.providerWarning", {
        provider: provider.providerId,
        status: provider.status,
      })
    : `${provider.providerId} · ${provider.model}`;
  return container;
}
