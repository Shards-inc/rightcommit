# Automate ITSM ticket classification and resolution · Pi + RAG interno

Il workflow elabora **tutti i ticket di un lotto**, prepara un piano per ciascuno e li applica
in sequenza con il connettore nativo ServiceNow. Pi classifica e cerca nel RAG interno;
Python prepara e valida i piani. Le credenziali restano nelle Personal Settings.

## Preparazione nella UI

1. Configurare Pi e ServiceNow nelle **Personal Settings**. Per ServiceNow usare URL e
   credenziali della PDI, non del portale Developer. Verificare la connessione e salvare.
2. In Storage indicizzare `examples/itsm/kb`: quattro articoli Markdown. Usare **keyword**
   senza download, oppure **hybrid / multilingual-minilm** per ricerca semantica.
3. Cliccare **Nuovo workflow** e scegliere **Automate ITSM ticket classification and resolution**
   oppure la variante **Northstar KB demo**. Aggiungere il nodo RAG
   con l’indice personale e collegarlo a **Pi ITSM operator** come contesto. Salvare.
4. Per le Request completare `config/servicenow.json` nella Source: route, item_sys_id,
   mapping dei campi e variabili obbligatorie. Non servono campi verticali nelle Personal Settings.
5. Avviare in modalità interattiva: l’approvazione del lotto mostra i piani di tutti i ticket.

Il generico è riutilizzabile con KB e regole del cliente; la demo Northstar contiene VPN,
installazione software, reset password e stato ticket. I cataloghi personali già salvati
non sono sovrascritti automaticamente dagli aggiornamenti dei template.

## Quali ticket vengono elaborati

Il file `tickets/batch.json` contiene un **namespace stabile** e da 1 a 100 ticket:

```json
{
  "namespace": "northstar-demo",
  "tickets": [
    {"request_id": "demo-incident-001", "chatInput": "La VPN non funziona: errore di connessione"},
    {"request_id": "demo-request-001", "chatInput": "Richiedo installazione software Python"}
  ]
}
```

Il file distribuito contiene anche FAQ e domanda sconosciuta: quattro ticket complessivi.
Per un nuovo lotto modificare i ticket nella Source configurata (una cartella personale
in sources per l’uso continuativo). Il namespace deve corrispondere a quello del file `config/servicenow.json`,
referenziato dal nodo Connection. Per un cliente diverso impostare esplicitamente il suo
namespace sia nel manifest sia nel file di configurazione. Non cambiare namespace o request_id per ritentare.
I request_id devono essere univoci nel lotto e stabili nel sistema che produce i ticket.

Pi legge ogni ticket come dato non attendibile, lo classifica Incident / Request / Other e
chiama rag_search separatamente per ciascuno. Salva `output/batch-analysis.json`, un oggetto
con una voce per request_id: ogni analisi contiene request_id, category e retrieval con
index, generation e i chunk pertinenti originali. Nessuna fonte inventata; results: [] se
non esiste una risposta pertinente. Una semplice corrispondenza RAG non prova la risoluzione.

```sh
python3 itsm.py batch --batch tickets/batch.json --analyses output/batch-analysis.json --config config/servicenow.json --out output/batch-plan.json
```

Il comando rifiuta ticket duplicati, analisi mancanti o riferite ad altri ticket. Genera
`output/batch-plan.json` con tutti i piani e `output/plans/<request_id>.json` per ciascuno.
Il nodo **Apply reviewed ITSM batch** usa `apply_ticket_batch` e legge il lotto completo.
Se manca il file, l’errore riporta `File piano mancante: output/batch-plan.json`.

## Applicazione e risultati

Dopo l’approvazione del contenuto esatto, il connettore elabora ogni elemento:

- **Incident:** crea un incident oppure recupera quello già associato alla sua identità.
- **Request:** sceglie la route dal ticket, risolve articolo e variabili dal file della Source
  oppure riusa la ricevuta dell’ordine precedente.
- **Other:** restituisce la risposta KB senza creare ticket. Senza fonti pertinenti segnala
  `needs_human`, da inoltrare al service desk.

L’assenza dell’articolo di catalogo blocca solo quella Request; gli altri ticket proseguono.
Anche un errore su un elemento non annulla gli elementi completati e non impedisce di
elaborare gli altri. Il report contiene una riga per request_id con azione, stato e numero /
sys_id effettivo quando disponibile. Gli stati sono `applied`, `reused`, `answered`,
`needs_human`, `blocked`, `attention_required`. Il completamento tecnico della run non
significa che le voci blocked/attention_required siano state applicate con successo.
La ricevuta completa è in `output/servicenow-apply-plan.json`.

## Come vengono evitati i duplicati

L’identità logica è **istanza ServiceNow + namespace + request_id**, non l’ID della run.
Le ricevute sono cifrate, persistenti e separate per utente. La prenotazione atomica della
ricevuta impedisce a due run della stessa app di inviare contemporaneamente lo stesso ticket.
Un nuovo avvio con gli stessi dati riusa i risultati già registrati. Per gli incidenti viene
usato anche un correlation_id stabile sulla PDI, per recuperarli dopo una risposta persa.
Le ricevute Incident riutilizzate vengono rilette sulla PDI per restituire lo stato attuale.

Se il testo originale, la categoria o la configurazione dell’ordine cambiano mantenendo lo
stesso ID già applicato, il connettore segnala un conflitto: serve un aggiornamento esplicito.
Un aggiornamento dei suggerimenti KB, da solo, non crea un nuovo ticket e non sovrascrive le
note di quello precedente. Per un evento realmente distinto il sistema sorgente assegna un
nuovo request_id; non assegnarlo solo per aggirare un errore.

Per un ordine catalogo con risposta persa, la ricevuta rimane da riconciliare: non viene
reinviato automaticamente perché non è possibile confermare con sicurezza l’assenza di un
ordine. La stessa regola vale per tentativi incident incerti senza record ancora visibile.
Controllare la PDI prima di una riconciliazione manuale. Conservare il volume dati dell’app:
contiene le ricevute di deduplicazione, particolarmente importanti per gli ordini catalogo.

Il passaggio da vecchie run prive di identità stabile richiede una riconciliazione esplicita:
non dedurre l’identità dal solo titolo. Nell’aggiornamento del workflow importato, il ticket
demo già verificato può essere associato alla nuova identità conservando l’INC esistente.

## Risoluzione verificata opzionale

La classificazione e l’applicazione non attestano che il problema tecnico sia risolto.
Per provare la risoluzione indicare nell’input della run **un solo request_id Incident** e
richiedere esplicitamente la review umana. Fornire prove concrete; dichiarare gli scenari
PDI simulati. Request, Other, elementi bloccati o incidenti già risolti/chiusi terminano
senza una nuova risoluzione. La risoluzione di tutti gli incidenti non è automatica.

Pi salva le prove umane in `output/resolution-evidence.txt` ed esegue:

```sh
python3 itsm.py resolution --created output/servicenow-apply-plan.json --request-id demo-incident-001 --evidence-file output/resolution-evidence.txt --out output/resolution.json
```

Il helper seleziona dal risultato nativo solo l’Incident applicato/riutilizzato per quell’ID.
Il connettore verifica sys_id, numero, request_id, namespace, run e nodo di provenienza prima
dell’approvazione e prima dell’invio. Non accetta il record letto da check_connection o un
record estraneo al lotto. La conferma finale mostra il PATCH esatto; ServiceNow deve
confermare state=6. Servono i permessi e le condizioni del record necessari alla risoluzione.
La ricevuta finale è in `output/servicenow-resolve-ticket.json`.

## File Python e setup ripetibile

- `itsm.py batch`: valida tutte le analisi Pi e genera il lotto e i singoli piani.
- `itsm.py plan`: resta disponibile per un singolo ticket; per esempio
  `python3 itsm.py plan --ticket tickets/incident.json --analysis output/pi-analysis.json --out output/plan.json`.
- `itsm.py resolution`: copia il destinatario dalla ricevuta nativa e aggiunge le prove umane.
- `setup_internal.py`: crea l’indice RAG interno e genera i workflow già collegati all’indice.
- `test_itsm.py`: verifica i contratti locali senza scrivere su ServiceNow.

Per il setup tramite API interna, configurare RIGHTCOMMIT_URL e RIGHTCOMMIT_SESSION con
una sessione autenticata, senza salvarla nel progetto o nei prompt, quindi:

```sh
python3 setup_internal.py --mode keyword
```

Il comando indicizza la KB, verifica una ricerca VPN e salva `workflow-generic-bound.json`
e `workflow-northstar-bound.json` in output/. Per KB cliente usare root sources e il percorso
relativo della cartella. Sono ammessi HTTPS o HTTP su loopback.

## Verifica

```sh
python3 -m unittest -v
```

I test Rust verificano deduplicazione tra run, ripresa dopo risposte perse, ordini mai
reinviati in caso di incertezza, blocco parziale, provenienza e approvazioni. Dal repository,
`scripts/itsm-internal-smoke.py` prova indicizzazione e import dei grafi con API locali reali.
I test simulati non sostituiscono il collaudo di provider Pi e PDI effettivamente configurati.
Gemini è sostituito da Pi e Qdrant dal RAG interno.

Due workflow separati per catalog Requests e requisiti software → Spec Kit sono in `../servicenow/`.
Guida completa: [CONFIGURING_SERVICENOW.md](../../guides/CONFIGURING_SERVICENOW.md).
