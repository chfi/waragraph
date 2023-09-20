

export async function addPathViewerEventHandlers(worker, path_viewer, canvas, overview) {
    console.log("adding path viewer event handlers & glue");

    const coord_sys = await path_viewer.coord_sys;
    console.log("coord_sys");
    console.log(coord_sys);

    const state = {
        dragging: false,
        dragOrigin: null,
    };

    const startDrag = (ev) => {
        state.dragging = true;
        state.dragOrigin = ev.clientX;
    };

    const stopDrag = (ev) => {
        state.dragging = false;
        state.dragOrigin = null;
    };

    canvas.addEventListener("mousedown", startDrag);
    canvas.addEventListener("mouseout", stopDrag);
    canvas.addEventListener("mouseup", stopDrag);

    canvas.addEventListener("mousemove", (event) => {
        let mx = event.clientX;

        path_viewer.getView().then((view) => {
            let { left, right } = view;
            let view_size = (right - left + 1);

            let bp_pos = left + (mx / canvas.width) * view_size;

            if (state.dragging === true) {
                let drag_delta = (state.dragOrigin - mx) / canvas.width;
                let del_bp = drag_delta * view_size;

                path_viewer.translateView(del_bp);
                state.dragOrigin = mx;
            }

        });
    });

    let last_view = null;
    const interval_id = setInterval(() => {
        path_viewer.getView().then((cur_view) => {

            let need_refresh;

            if (last_view === null) {
                // console.log("last view null");
                need_refresh = true;
            } else {
                let views_equal = last_view.left == cur_view.left
                    && last_view.right == cur_view.right;
                // console.log("views equal: " + views_equal);

                // console.log(last_view);
                // console.log(cur_view);

                need_refresh = !views_equal;
            };

            if (need_refresh) {
                // console.log("left: " + cur_view.left + ", right: " + cur_view.right);
                path_viewer.sample();
                overview.draw(cur_view);
                last_view = cur_view;
            }

        });

    }, 50);
        
}
