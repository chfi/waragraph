export function setTransferHandlers(rxjs, Comlink) {
    const { Observable, Observer, Subscribable, Subscription } = rxjs;
    
    Comlink.transferHandlers.set("observable", {
        canHandle: (val) => {
            console.log("in observable canHandle()");
            return val instanceof Observable;
        },
        deserialize: (val) => {
            console.log(" ~~~~~~~~~~#  in observable deserialize()");
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
            console.log(" #~~~~~~~~~~~  in observable serialize()");
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

}
