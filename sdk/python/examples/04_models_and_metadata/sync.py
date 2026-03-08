from codex_app_server import Codex

with Codex() as codex:
    print("metadata:", codex.metadata)

    models = codex.models()
    print("models.count:", len(models.data))
    if models.data:
        print("first model id:", models.data[0].id)
