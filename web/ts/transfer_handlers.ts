import { DataType, Field, RecordBatch, Schema, Struct, Table, makeData, makeVector } from "apache-arrow";
import { fieldFromJSON, schemaFromJSON } from "apache-arrow/ipc/metadata/json";
import { JSONTypeAssembler } from "apache-arrow/visitor/jsontypeassembler";
import { CoordSysArrow } from "./coordinate_system";

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
    
    Comlink.transferHandlers.set('table', {
        canHandle: (value) => {
            return value instanceof Table;
        },
        deserialize: (value) => {
          console.warn("deserializing");
          console.warn(value);

          console.warn(`deserializing schema ${value.schema}`);
          console.warn(value.schema);

          // console.warn
          const fields = schemaFieldsFromJSON(value.schema, value.dictionaries);
          const metadata = customMetadataFromJSON(value.metadata);

          console.warn("fields");
          console.warn(fields);
          console.warn(metadata);

          const schema = new Schema(fields, metadata, value.dictionaries);
          console.warn(schema);

          const batches: RecordBatch[] = [];
          console.warn(value.batches);
          for (const batch of value.batches) {
            console.warn(batch);
            console.warn(batch.schema);
            console.warn(batch.data);

            batches.push(new RecordBatch(schema, batch.data));
          }
        
          return new Table(value.schema, batches, value._offsets);
        },
      serialize: (value: Table) => {
          console.warn("serializing");
          console.warn(value);

        console.warn(`JSON field: ${JSON.stringify(fieldToJSON(value.schema.fields[0]))}`);

        console.warn("original field");
        console.warn(value.schema.fields[0]);
        const field = fieldToJSON(value.schema.fields[0]);
        console.warn(field);

        console.warn("fieldFromJSON");
        console.warn(fieldFromJSON(field));

        console.warn(value.schema);
        const schema = 
          { fields: value.schema.fields.map(field => fieldToJSON(field)),
            metadata: value.schema.metadata,
            dictionaries: value.schema.dictionaries
          };
        console.warn("stringified schema: ", schema);

          // @ts-ignore
          // const result = { schema: value.schema, batches: value.batches, _offsets: value._offsets };
          const result = { schema, batches: value.batches, _offsets: value._offsets };
          console.warn(result);
          return [
            result,
            []
          ];
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
