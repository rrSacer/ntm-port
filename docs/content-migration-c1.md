# Migração C1: durabilidade por componentes

O pacote de conteúdo passa a declarar a durabilidade dos **26 itens duráveis** por `"components": {"minecraft:max_damage": N}`. O campo `max_damage` na raiz da declaração não é mais emitido: o upstream atual o recusa. O catálogo `ids.json` conserva `max_damage` como dado de origem para as regras de máquinas; ele não é a declaração carregada pelo motor.

`tools/content-gen/generate.py --max-damage` emite o componente. `tools/build-visual.sh` aplica o mesmo formato ao pacote de produção e remove qualquer campo antigo residual. O modo sem a flag continua produzindo declarações sem durabilidade. Matrizes sem desgaste e lâminas de desh não recebem o componente. A condição de empilhamento 1 continua obrigatória.

Foi reaproveitado o suporte oficial a componentes do upstream e o pipeline de empacotamento existente, sem um formato de compatibilidade próprio. O teste existente do gerador cobre lâminas de aço/titânio/desh e agora também a matriz `stamp_357`, com e sem a flag; ele proíbe o campo antigo na raiz.

A C1 foi preparada sobre uma exportação limpa do HEAD de 2.695 IDs, preservando o lote 4 de energia ainda em edição. Os templates das bombas pertencem à C2/L4 e não mudam nesta etapa.

Verificação da primeira entrega:

- 162 testes Python passaram em cada seed (`PYTHONHASHSEED=1/42`), em exportação isolada; os validadores de conteúdo e receitas passaram.
- As 26 declarações foram conferidas contra os limites de `ids.json`, sem campo antigo na raiz. Pacote de 6.342.200 bytes, SHA-256 `b5bf00131594c2cec40a67fa08d3baf145b2afb930a1058a1cf4f0c58cd9fe22`.
- **Check upstream passou**, no binário de `migracao-upstream`.
- **Check do fork e prova em jogo bloqueados** no binário `horizon-patched` recompilado às 10:16 de 30/10: o loader ainda recusa `components` e espera `max_damage`. A prova das prensas foi interrompida porque o mod não carregou; nenhum desgaste foi validado nessa tentativa. O agente de motor foi acionado para aceitar o componente no fork e recompilar; repetir check e prova após esse retorno.

Evidências: `content-gen-evidence/migration-c1/` (`package.json`, `tests-1.log`, `tests-42.log`, `validate-bulk.log`, `validate-recipes.log`, `check-upstream.log`, `check-fork.log`, `rejected-old-fork-client.log`). A C1 permanece pendente da aceitação do fork e da prova de desgaste.
