# Edge cases: reserved marketplace names (HIL fix, 2.5.1)

| ID | Category | Case | Expected |
|---|---|---|---|
| ECH1-01 | Boundary | The reserved repo's plugin lives in a subfolder (`./plugins/<name>`) | The wrapper points at that subfolder of the clone |
