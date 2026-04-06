declare module 'dkg.js' {
  export default class DKG {
    constructor(config: any);
    asset: {
      create(data: any, options?: any): Promise<{ UAL: string }>;
      get(ual: string): Promise<any>;
      update(ual: string, data: any): Promise<any>;
    };
    graph: {
      query(sparql: string, type: string): Promise<{ data: any[] }>;
    };
    node: {
      info(): Promise<any>;
    };
  }
}
