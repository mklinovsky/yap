import { useEffect, useState } from "react";
import { api, type Settings as SettingsData } from "../api";
import { SaveBar } from "../components/SaveBar";
import { useSaveState } from "../components/useSaveState";
import { AppearanceGroup } from "./AppearanceGroup";
import { DictationGroup } from "./DictationGroup";
import { TranscriptionGroup } from "./TranscriptionGroup";

export function Settings() {
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [keyPreview, setKeyPreview] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [vocabulary, setVocabulary] = useState("");
  const [openAtLogin, setOpenAtLogin] = useState(false);
  const [saved, setSaved] = useState<{
    settings: SettingsData;
    vocabulary: string;
    openAtLogin: boolean;
  } | null>(null);

  useEffect(() => {
    Promise.all([api.getSettings(), api.getOpenAtLogin()]).then(([loaded, loginItem]) => {
      const loadedVocabulary = loaded.keywords.join(", ");
      setSettings(loaded);
      setVocabulary(loadedVocabulary);
      setOpenAtLogin(loginItem);
      setSaved({ settings: loaded, vocabulary: loadedVocabulary, openAtLogin: loginItem });
    });
    api.apiKeyPreview().then(setKeyPreview);
  }, []);

  const dirty =
    apiKey !== "" ||
    vocabulary !== saved?.vocabulary ||
    openAtLogin !== saved?.openAtLogin ||
    JSON.stringify(settings) !== JSON.stringify(saved?.settings);
  const { status, onSubmit } = useSaveState(dirty);

  if (!settings) {
    return null;
  }

  const update = (patch: Partial<SettingsData>) => setSettings({ ...settings, ...patch });

  const save = async () => {
    const keywords = vocabulary
      .split(",")
      .map((term) => term.trim())
      .filter(Boolean);
    const next = { ...settings, keywords };
    await api.saveSettings(next);
    if (apiKey) {
      await api.setApiKey(apiKey);
      setApiKey("");
      setKeyPreview(await api.apiKeyPreview());
    }
    if (openAtLogin !== saved?.openAtLogin) {
      await api.setOpenAtLogin(openAtLogin);
    }
    const nextVocabulary = keywords.join(", ");
    setSettings(next);
    setVocabulary(nextVocabulary);
    setSaved({ settings: next, vocabulary: nextVocabulary, openAtLogin });
  };

  return (
    <form className="settings-page" onSubmit={onSubmit(save)}>
      <div className="settings-scroll">
        <div className="settings">
          <TranscriptionGroup
            settings={settings}
            apiKey={apiKey}
            keyPreview={keyPreview}
            vocabulary={vocabulary}
            onChange={update}
            onApiKeyChange={setApiKey}
            onVocabularyChange={setVocabulary}
          />
          <DictationGroup
            settings={settings}
            openAtLogin={openAtLogin}
            onChange={update}
            onOpenAtLoginChange={setOpenAtLogin}
          />
          <AppearanceGroup settings={settings} onChange={update} />
        </div>
      </div>
      <SaveBar status={status} dirty={dirty} />
    </form>
  );
}
