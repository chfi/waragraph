import { DataType, Field, RecordBatch, Schema, Struct, Table, makeData, makeVector } from "apache-arrow";
import { fieldFromJSON, schemaFromJSON } from "apache-arrow/ipc/metadata/json";
import { JSONTypeAssembler } from "apache-arrow/visitor/jsontypeassembler";
import { CoordSysArrow } from "./coordinate_system";
import { GraphLayoutTable } from "./graph_layout";

export function setTransferHandlers(rxjs, Comlink) {
    const { Observable, Observer, Subscribable, Subscription } = rxjs;
    
    Comlink.transferHandlers.set("observable", {
        canHandle: (val) => {
            return val instanceof Observable;
        },
        deserialize: (val) => {
            return new Observable((observer) => {
                const remote = Comlink.transferHandlers.get('proxy')
                      .deserialize(val);

                remote.subscribe(Comlink.proxy({
                    next: (next) => observer.next(next),
                    error: (error) => observer.error(error),
                    complete: () => observer.complete(),
                })).then((subscription) => observer.add(() => {
                    subscription.unsubscribe();
                    remote[Comlink.releaseProxy]();
                }));
            });
        },
        serialize: (val) => {
            return Comlink.transferHandlers.get('proxy').serialize({
                subscribe: (observer) => val.subscribe({
                    next: (next) => void observer.next(next).then(),
                    error: (error) => void observer.error(error).then(),
                    complete: () => void observer.complete().then(),
                })
            });
        }
    });

    Comlink.transferHandlers.set('subscription', {
        canHandle: (value) => {
            return value instanceof Subscription;
        },
        deserialize: (value) => {
            return new Subscription(() => {
                const remote = Comlink.transferHandlers.get('proxy')
                      .deserialize(value);

                void remote.unsubscribe().then(() => {
                    remote[releaseProxy]();
                });
            });
        },
        serialize: (value) => {
            return Comlink.transferHandlers.get('proxy').serialize({
                unsubscribe: () => value.unsubscribe()
            });
        }
    });

  Comlink.transferHandlers.set('CoordSysArrow', {
    canHandle: (value) => {
      return value instanceof CoordSysArrow;
    },
    serialize: (csys: CoordSysArrow) => {
      const node_order = csys.node_order.data.map((data) => data.values);
      const step_offsets = csys.step_offsets.data.map((data) => data.values);

      return [{ step_offsets, node_order }, ];
    },
    deserialize: (value) => {
      return new CoordSysArrow(value.node_order, value.step_offsets);
    }
  });

  Comlink.transferHandlers.set('GraphLayoutTable', {
    canHandle: (value) => {
      return value instanceof GraphLayoutTable;
    },
    serialize: (layout: GraphLayoutTable) => {
      const { aabb_min, aabb_max } = layout;

      const x = layout.x.data.map((data) => data.values);
      const y = layout.y.data.map((data) => data.values);

      return [{ x, y, aabb_min, aabb_max }, ];
    },
    deserialize: ({ x, y, aabb_min, aabb_max }) => {
      return new GraphLayoutTable(x, y, aabb_min, aabb_max);
    }
  });
    

}


// taken from https://github.com/apache/arrow/blob/a690088193711447aa4d526f2257027f9a459efa/js/src/ipc/writer.ts#L56
function fieldToJSON({ name, type, nullable }: Field): Record<string, unknown> {
    const assembler = new JSONTypeAssembler();
    return {
        'name': name, 'nullable': nullable,
        'type': assembler.visit(type),
        'children': (type.children || []).map((field: any) => fieldToJSON(field)),
        'dictionary': !DataType.isDictionary(type) ? undefined : {
            'id': type.id,
            'isOrdered': type.isOrdered,
            'indexType': assembler.visit(type.indices)
        }
    };
}

function schemaFieldsFromJSON(_schema: any, dictionaries?: Map<number, DataType>) {
    return (_schema['fields'] || []).filter(Boolean).map((f: any) => Field.fromJSON(f, dictionaries));
}

function customMetadataFromJSON(metadata: { key: string; value: string }[] = []) {
    return new Map<string, string>(metadata.map(({ key, value }) => [key, value]));
}
