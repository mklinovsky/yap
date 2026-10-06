import { useEffect, useState } from "react";
import { api, type Transformation, type Transformations as TransformationsData } from "../api";
import { SaveBar } from "../components/SaveBar";
import { useSaveState } from "../components/useSaveState";
import { ConnectionGroup } from "./ConnectionGroup";
import { TransformationEditor } from "./TransformationEditor";
import { TransformationList } from "./TransformationList";

export function Transformations() {
  const [draft, setDraft] = useState<TransformationsData | null>(null);
  const [saved, setSaved] = useState<TransformationsData | null>(null);
  const [transcriptionUrl, setTranscriptionUrl] = useState("");
  const [keyPreview, setKeyPreview] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [editing, setEditing] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.getTransformations(), api.getSettings()]).then(([loaded, settings]) => {
      setDraft(loaded);
      setSaved(loaded);
      setTranscriptionUrl(settings.baseUrl);
    });
    api.transformApiKeyPreview().then(setKeyPreview);
  }, []);

  const dirty = apiKey !== "" || JSON.stringify(draft) !== JSON.stringify(saved);
  const { status, onSubmit } = useSaveState(dirty);

  if (!draft) {
    return null;
  }

  const update = (patch: Partial<TransformationsData>) => setDraft({ ...draft, ...patch });
  const updateItem = (id: string, patch: Partial<Transformation>) =>
    update({ items: draft.items.map((item) => (item.id === id ? { ...item, ...patch } : item)) });

  const save = async () => {
    const next = { ...draft, baseUrl: draft.baseUrl ?? transcriptionUrl };
    await api.saveTransformations(next);
    if (apiKey) {
      await api.setTransformApiKey(apiKey);
      setApiKey("");
      setKeyPreview(await api.transformApiKeyPreview());
    }
    setDraft(next);
    setSaved(next);
  };

  const add = () => {
    const item: Transformation = {
      id: crypto.randomUUID(),
      name: "New transformation",
      systemPrompt: "",
      userTemplate: "",
      model: null,
      shortcut: null,
    };
    update({ items: [...draft.items, item] });
    setEditing(item.id);
  };

  const remove = (id: string) => {
    update({ items: draft.items.filter((item) => item.id !== id) });
    setEditing(null);
  };

  const editingItem = draft.items.find((item) => item.id === editing);

  return (
    <form className="settings-page" onSubmit={onSubmit(save)}>
      <div className="settings-scroll">
        <div className="settings">
          {editingItem ? (
            <TransformationEditor
              item={editingItem}
              defaultModel={draft.defaultModel}
              onChange={(patch) => updateItem(editingItem.id, patch)}
              onBack={() => setEditing(null)}
            />
          ) : (
            <>
              <ConnectionGroup
                transformations={draft}
                transcriptionUrl={transcriptionUrl}
                apiKey={apiKey}
                keyPreview={keyPreview}
                onChange={update}
                onApiKeyChange={setApiKey}
              />
              <TransformationList items={draft.items} onEdit={setEditing} onAdd={add} />
            </>
          )}
        </div>
      </div>
      <SaveBar status={status} dirty={dirty}>
        {editingItem && (
          <button
            type="button"
            className="text-button danger"
            onClick={() => remove(editingItem.id)}
          >
            Delete transformation
          </button>
        )}
      </SaveBar>
    </form>
  );
}
