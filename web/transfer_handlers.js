export function setTransferHandlers(rxjs, Comlink) {
    const { Observable, Observer, Subscribable, Subscription } = rxjs;

    Comlink.transferHandlers.set("observable", {
        canHandle(val) {
            return val instanceof Observable;
        },
        serialize(val) {
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
        deserialize(val) {
            return Comlink.transferHandlers.get('proxy').serialize({
                subscribe: (observer) => val.subscribe({
                    next: (next) => observer.next(next).then(),
                    error: (error) => observer.error(error).then(),
                    complete: () => observer.complete().then(),
                })
            });
        }
    });
}
