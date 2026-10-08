# Gera a fixture dos testes da F7 (busca, "onde conseguir", destaque na tela Bosses) a partir das
# respostas reais do questlog salvas em baixado-F7. Nada é digitado: cada valor sai do arquivo baixado,
# só com os campos que os testes leem. Uso:
#   python -I -X utf8 gerar_fixture.py <baixado-F7> <getItem antigo com muitos droppers> <saida.json>
# O segundo argumento é o getItem do Peitoral da Fantasia (210130005) baixado em 2026-10-06 numa sessão
# anterior (scratchpad/itens/210130005.json): 281 NPCs, 20 deles chefes de Altgard a 0,08%.
import json, sys, os, time

baixado, antigo, saida = sys.argv[1], sys.argv[2], sys.argv[3]

def dados(nome):
    with open(os.path.join(baixado, nome + ".json"), encoding="utf-8") as f:
        return json.load(f)["result"]["data"]

IDENTIDADE = ("id", "name", "icon", "grade", "mainCategory", "subCategory", "dbType")
RELACOES = ("itemIsDroppedByNpcs", "itemIsContainedInItems", "itemContainsItems", "itemIsOutputOfRecipes",
            "itemIsRewardOfQuests", "itemIsRewardOfDungeons", "itemIsRewardOfAchievements", "itemIsSoldByNpcs",
            "itemIsObtainedFromGatherables", "itemIsRewardOfSupplyRequests", "itemIsRewardOfDaevaPasses")

def item(d, droppers=None):
    """Identidade e listas de origem; `droppers`: códigos de NPC que ficam (None = todos)."""
    out = {k: d[k] for k in IDENTIDADE if k in d}
    for k in RELACOES:
        if k in d:
            lista = d[k]
            if k == "itemIsDroppedByNpcs" and droppers is not None:
                lista = [n for n in lista if n["id"] in droppers]
            out[k] = lista
    return out

regiao = dados("21-regiao-altgard")
chefes = {n["id"] for n in regiao["regionHasNpcs"]}
with open(antigo, encoding="utf-8") as f:
    fantasia = json.load(f)["result"]["data"]
# Os chefes de Altgard e os 3 mobs comuns de maior chance: o teste da chance mínima precisa dos dois.
comuns = [n["id"] for n in sorted(fantasia["itemIsDroppedByNpcs"], key=lambda n: -(n.get("chance") or 0))
          if n["id"] not in chefes][:3]

npc = dados("23-npc-fada-contaminada")
fixture = {
    "_origem": f"Gerado por gerar_fixture.py em {time.strftime('%Y-%m-%d')} a partir das respostas do questlog "
               "(baixado-F7, 2026-10-08; Peitoral da Fantasia de 2026-10-06). Não editar à mão.",
    "busca_newbold": dados("01-busca-newbold"),
    "regiao_altgard": {k: regiao[k] for k in ("id", "name", "regionHasNpcs")},
    "luvas_newbold_do_chefe": item(dados("16-item-luvas-newbold")),
    "luvas_newbold_do_bau": item(dados("28-item-luvas-newbold-129")),
    "bau_newbold": item(dados("30-item-bau-newbold")),
    "pedra_de_mana_craft": item(dados("17-item-craft-pedra-mana")),
    "receita_pedra_de_mana": {k: v for k, v in dados("19-receita-pedra-mana").items() if k != "icon"},
    "npc_fada": {k: npc[k] for k in ("id", "name", "level", "npcSubType", "isNamed", "npcIsFoundInRegions")},
    "peitoral_fantasia": item(fantasia, droppers=chefes | set(comuns)),
}
with open(saida, "w", encoding="utf-8") as f:
    json.dump(fixture, f, ensure_ascii=False, indent=1)

# Conferência: o que os testes vão afirmar, calculado do mesmo dado.
def chefes_com(item_d, minimo=0.0):
    return sorted((n["name"], n.get("chance")) for n in item_d.get("itemIsDroppedByNpcs", [])
                  if n["id"] in chefes and (n.get("chance") or 0) >= minimo)

f = fixture
print("bytes", os.path.getsize(saida))
print("luvas 076 -> chefes:", chefes_com(f["luvas_newbold_do_chefe"]))
bau = f["luvas_newbold_do_bau"]["itemIsContainedInItems"]
print("luvas 129 -> baús:", [(b["id"], b["chance"]) for b in bau], "-> chefes do baú:", chefes_com(f["bau_newbold"]))
print("peitoral fantasia: chefes com chance >= 0", len(chefes_com(f["peitoral_fantasia"])),
      "; >= 0,5%", len(chefes_com(f["peitoral_fantasia"], 0.005)))
r = f["pedra_de_mana_craft"]["itemIsOutputOfRecipes"]
print("pedra de mana: receitas", [(x["id"], x["mainCategory"]) for x in r],
      "entradas da 1ª", [(e["name"], e["quantity"]) for e in r[0]["recipeInputItems"]])
rc = f["receita_pedra_de_mana"]
print("receita: profissão", rc["mainCategory"], "maestria", rc["masteryGrade"], rc["masteryLevel"], "raça", rc["qualificationRace"])
print("fada: regiões", [(x["name"], x["count"]) for x in f["npc_fada"]["npcIsFoundInRegions"]])
nomes = {}
for x in f["busca_newbold"]["pageData"]:
    nomes.setdefault(x["name"], []).append(x["id"])
print("busca newbold: nomes repetidos", {k: v for k, v in nomes.items() if len(v) > 1})
