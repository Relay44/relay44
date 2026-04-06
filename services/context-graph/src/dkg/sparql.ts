const MAX_LIMIT = 50;

function clamp(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(val, max));
}

function escape(str: string): string {
  return str
    .replace(/[\\"']/g, '\\$&')
    .replace(/\n/g, '\\n')
    .replace(/\r/g, '\\r')
    .replace(/\t/g, '\\t')
    .slice(0, 256);
}

// --- Market queries ---

export function queryMarketByConditionId(conditionId: string): string {
  const safe = escape(conditionId);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?market ?name ?description ?category ?dateCreated ?expires ?odds ?volume ?liquidity
    WHERE {
      ?market a schema:FinancialProduct ;
              schema:name ?name .
      ?market schema:additionalProperty ?condProp .
      ?condProp schema:name "conditionId" ; schema:value "${safe}" .
      OPTIONAL { ?market schema:description ?description }
      OPTIONAL { ?market schema:category ?category }
      OPTIONAL { ?market schema:dateCreated ?dateCreated }
      OPTIONAL { ?market schema:expires ?expires }
      OPTIONAL {
        ?market schema:additionalProperty ?oddsProp .
        ?oddsProp schema:name "currentOdds" ; schema:value ?odds .
      }
      OPTIONAL {
        ?market schema:additionalProperty ?volProp .
        ?volProp schema:name "volume24h" ; schema:value ?volume .
      }
      OPTIONAL {
        ?market schema:additionalProperty ?liqProp .
        ?liqProp schema:name "totalLiquidity" ; schema:value ?liquidity .
      }
    }
    LIMIT 1
  `;
}

// --- Claim queries ---

export function queryClaimsByMarket(conditionId: string, limit = 20): string {
  const safe = escape(conditionId);
  const safeLimit = clamp(limit, 1, MAX_LIMIT);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?claim ?text ?claimHash ?confidence ?sentiment ?verificationStatus ?date
    WHERE {
      ?snapshot a schema:Dataset ;
               schema:hasPart ?claim .
      ?snapshot schema:additionalProperty ?condProp .
      ?condProp schema:name "conditionId" ; schema:value "${safe}" .
      ?claim a schema:Claim ;
             schema:text ?text ;
             schema:dateCreated ?date .
      OPTIONAL {
        ?claim schema:additionalProperty ?hashProp .
        ?hashProp schema:name "claimHash" ; schema:value ?claimHash .
      }
      OPTIONAL {
        ?claim schema:additionalProperty ?confProp .
        ?confProp schema:name "confidence" ; schema:value ?confidence .
      }
      OPTIONAL {
        ?claim schema:additionalProperty ?sentProp .
        ?sentProp schema:name "sentiment" ; schema:value ?sentiment .
      }
      OPTIONAL {
        ?claim schema:additionalProperty ?verProp .
        ?verProp schema:name "verificationStatus" ; schema:value ?verificationStatus .
      }
    }
    ORDER BY DESC(?date)
    LIMIT ${safeLimit}
  `;
}

export function queryClaimByHash(claimHash: string): string {
  const safe = escape(claimHash);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?claim ?text ?confidence ?sentiment ?verificationStatus ?date
    WHERE {
      ?claim a schema:Claim .
      ?claim schema:additionalProperty ?hashProp .
      ?hashProp schema:name "claimHash" ; schema:value "${safe}" .
      OPTIONAL { ?claim schema:text ?text }
      OPTIONAL { ?claim schema:dateCreated ?date }
      OPTIONAL {
        ?claim schema:additionalProperty ?confProp .
        ?confProp schema:name "confidence" ; schema:value ?confidence .
      }
      OPTIONAL {
        ?claim schema:additionalProperty ?sentProp .
        ?sentProp schema:name "sentiment" ; schema:value ?sentiment .
      }
      OPTIONAL {
        ?claim schema:additionalProperty ?verProp .
        ?verProp schema:name "verificationStatus" ; schema:value ?verificationStatus .
      }
    }
    LIMIT 1
  `;
}

// --- Source queries ---

export function querySourcesByCredibility(minCredibility: number, limit = 20): string {
  const safeMin = clamp(minCredibility, 0, 100);
  const safeLimit = clamp(limit, 1, MAX_LIMIT);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?source ?name ?url ?platform ?credibility ?date
    WHERE {
      {
        ?source a schema:NewsArticle ;
                schema:name ?name ;
                schema:url ?url ;
                schema:datePublished ?date .
      } UNION {
        ?source a schema:SocialMediaPosting ;
                schema:name ?name ;
                schema:url ?url ;
                schema:datePublished ?date .
      }
      ?source schema:additionalProperty ?credProp .
      ?credProp schema:name "sourceCredibility" ; schema:value ?credibility .
      FILTER(?credibility >= ${safeMin})
      OPTIONAL {
        ?source schema:additionalProperty ?platProp .
        ?platProp schema:name "platform" ; schema:value ?platform .
      }
    }
    ORDER BY DESC(?credibility)
    LIMIT ${safeLimit}
  `;
}

// --- Narrative queries ---

export function queryNarrativeEdgesByMarket(conditionId: string, limit = 50): string {
  const safe = escape(conditionId);
  const safeLimit = clamp(limit, 1, MAX_LIMIT);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?edge ?sourceNode ?targetNode ?relType ?weight ?velocity ?date
    WHERE {
      ?snapshot a schema:Dataset ;
               schema:hasPart ?edge .
      ?snapshot schema:additionalProperty ?condProp .
      ?condProp schema:name "conditionId" ; schema:value "${safe}" .
      ?edge a schema:Action ;
            schema:name "NarrativeSpread" ;
            schema:agent ?sourceRef ;
            schema:object ?targetRef ;
            schema:startTime ?date .
      BIND(STR(?sourceRef) as ?sourceNode)
      BIND(STR(?targetRef) as ?targetNode)
      OPTIONAL {
        ?edge schema:additionalProperty ?relProp .
        ?relProp schema:name "relationshipType" ; schema:value ?relType .
      }
      OPTIONAL {
        ?edge schema:additionalProperty ?wProp .
        ?wProp schema:name "weight" ; schema:value ?weight .
      }
      OPTIONAL {
        ?edge schema:additionalProperty ?velProp .
        ?velProp schema:name "spreadVelocity" ; schema:value ?velocity .
      }
    }
    ORDER BY DESC(?date)
    LIMIT ${safeLimit}
  `;
}

// --- Snapshot queries ---

export function querySnapshotsByMarket(conditionId: string, limit = 10): string {
  const safe = escape(conditionId);
  const safeLimit = clamp(limit, 1, MAX_LIMIT);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?snapshot ?score ?pattern ?flags ?date ?claimCount ?sourceCount
    WHERE {
      ?snapshot a schema:Dataset ;
               schema:dateCreated ?date .
      ?snapshot schema:additionalProperty ?condProp .
      ?condProp schema:name "conditionId" ; schema:value "${safe}" .
      OPTIONAL {
        ?snapshot schema:additionalProperty ?scoreProp .
        ?scoreProp schema:name "misinfoScore" ; schema:value ?score .
      }
      OPTIONAL {
        ?snapshot schema:additionalProperty ?patProp .
        ?patProp schema:name "narrativePattern" ; schema:value ?pattern .
      }
      OPTIONAL {
        ?snapshot schema:additionalProperty ?flagProp .
        ?flagProp schema:name "anomalyFlags" ; schema:value ?flags .
      }
      OPTIONAL {
        ?snapshot schema:additionalProperty ?claimProp .
        ?claimProp schema:name "claimCount" ; schema:value ?claimCount .
      }
      OPTIONAL {
        ?snapshot schema:additionalProperty ?srcProp .
        ?srcProp schema:name "sourceCount" ; schema:value ?sourceCount .
      }
    }
    ORDER BY DESC(?date)
    LIMIT ${safeLimit}
  `;
}

export function queryMisinfoScoreHistory(conditionId: string, limit = 30): string {
  const safe = escape(conditionId);
  const safeLimit = clamp(limit, 1, MAX_LIMIT);
  return `
    PREFIX schema: <https://schema.org/>
    SELECT ?date ?score
    WHERE {
      ?snapshot a schema:Dataset ;
               schema:dateCreated ?date .
      ?snapshot schema:additionalProperty ?condProp .
      ?condProp schema:name "conditionId" ; schema:value "${safe}" .
      ?snapshot schema:additionalProperty ?scoreProp .
      ?scoreProp schema:name "misinfoScore" ; schema:value ?score .
    }
    ORDER BY ASC(?date)
    LIMIT ${safeLimit}
  `;
}
