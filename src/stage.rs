//! Stage struct
use crate::World;
use crate::system::{System, Run, Dependencies, End, Errors};
use std::collections::HashMap;
use std::any::{TypeId};
use std::cell::{RefCell, Ref, RefMut};
use std::fmt::{Debug, Formatter};
use std::option::Option::Some;

struct SystemInfo {
    dependencies : Vec<TypeId>,
    is_active : bool,
    is_once : bool,
    system : RefCell<Box<dyn Run>>,
}

/// Stage = World + Systems
pub struct Stage{
    world : RefCell<World>,
    systems : HashMap<TypeId,SystemInfo>,
    need_update : bool,
    run_queue : Vec<TypeId>,
    need_init : Vec<TypeId>
}

impl Debug for Stage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f
            .debug_struct("Stage")
            .field("world",&self.world)
            .field("systems id",&self.run_queue)
            .finish()
    }
}

impl Stage {
    /// Create a stage with a empty world.
    pub fn new() -> Stage {
        let mut stage = Stage {
            world: RefCell::new(World::new()),
            systems: HashMap::new(),
            need_update: false,
            run_queue: vec![],
            need_init: vec![]
        };
        stage.add_system(Errors::new());
        stage.deactivate::<Errors>();
        stage
    }

    /// Create a stage with determined world.
    pub fn from_world(world : World) -> Stage {
        let mut stage = Stage {
            world : RefCell::new(world),
            systems : HashMap::new(),
            need_update: false,
            run_queue: vec![],
            need_init: vec![]
        };
        stage.add_system(Errors::new());
        stage.deactivate::<Errors>();
        stage
    }
    /// Add a normal system in stage.
    pub fn add_system<T : for<'a> System<'a>>(&mut self,system : T) -> &mut Self{
        self.need_update = true;
        self.need_init.push(TypeId::of::<T>());
        self.systems.insert(
            TypeId::of::<T>(),
            SystemInfo {
                dependencies: <<T as System>::Dependencies as Dependencies>::dependencies(),
                is_active: true,
                is_once : false,
                system : RefCell::new(Box::new(system)),
            }
        );
        self.system_data_mut::<Errors>().register::<T>();
        self
    }

    /// Add a system that run only once in stage.
    #[deprecated = "Use System::init() !"]
    pub fn add_once_system<T : for<'a> System<'a>>(&mut self,system : T) -> &mut Self{
        self.need_update = true;
        self.need_init.push(TypeId::of::<T>());
        self.systems.insert(
            TypeId::of::<T>(),
            SystemInfo {
                dependencies: <<T as System>::Dependencies as Dependencies>::dependencies(),
                is_active: true,
                is_once : true,
                system: RefCell::new(Box::new(system)),
            }
        );
        self.system_data_mut::<Errors>().register::<T>();
        self
    }

    /// Check if stage has such system.
    pub(in crate) fn has_system<T : for<'a> System<'a>>(&self) -> bool {
        self.has_system_dyn(TypeId::of::<T>())
    }


    /// Check if stage has such system from a dynamic TypeId
    pub(in crate) fn has_system_dyn(&self,system : TypeId) -> bool {
        self.systems.contains_key(&system)
    }

    /// Deactivate a system.
    /// ### Detail
    /// * A deactivated system will not be executed in stage run.
    /// * The depended systems also will not be executed too.
    pub fn deactivate<T : for<'a> System<'a>>(&mut self) -> &mut Self{
        self.deactivate_dyn(TypeId::of::<T>())
    }

    /// Same as ```deactivate```
    pub fn deactivate_dyn(&mut self,system : TypeId) -> &mut Self{
        debug_assert!(self.has_system_dyn(system),
                      "There is no such system in stage");
        self.systems
            .get_mut(&system)
            .unwrap()
            .is_active = false;
        self
    }

    /// Activate a system.
    /// ### Detail
    /// The system is activated by default.
    pub fn activate<T : for<'a> System<'a>>(&mut self) -> &mut Self {
        self.activate_dyn(TypeId::of::<T>())
    }

    /// Same as ```activate```
    pub fn activate_dyn(&mut self,system : TypeId) -> &mut Self {
        debug_assert!(self.has_system_dyn(system),
                      "There is no such system in stage");
        self.systems
            .get_mut(&system)
            .unwrap()
            .is_active = true;
        self
    }

    /// Get a reference of System data.
    pub fn system_data_ref<T : for<'a> System<'a>>(&self) -> Ref<'_,T> {
        debug_assert!(self.has_system::<T>(),
                    "There is no such system in stage");
        let any = &self.systems
            .get(&TypeId::of::<T>())
            .unwrap()
            .system;
        let any = any.borrow();
        Ref::map(any,|any| unsafe {
            any.downcast_ref::<T>()
        })
    }


    /// Get a mutable reference of System data.
    pub fn system_data_mut<T : for<'a> System<'a>>(&self) -> RefMut<'_,T> {
        debug_assert!(self.has_system::<T>(),
                      "There is no such system in stage");
        let any = &self.systems
            .get(&TypeId::of::<T>())
            .unwrap()
            .system;
        let any = any.borrow_mut();
        RefMut::map(any,|any| unsafe {
            any.downcast_mut::<T>()
        })
    }

    /// Get a reference of world in stage.
    pub fn world_ref(&self) -> Ref<'_,World> {
        self.world.borrow()
    }

    /// Get a mutable reference of world in stage.
    pub fn world_mut(&self) -> RefMut<'_,World> {
        self.world.borrow_mut()
    }

    /// Dynamically add a dependency to the system
    /// ### Panics
    /// * Panic if the source system is not in stage
    /// * Panic if the dependency system has already in source system
    pub fn add_dependency<Src,Dep>(&mut self)
        where Src : for<'a> System<'a>,
              Dep : for<'a> System<'a> {
        debug_assert!(self.has_system::<Src>(),
            "Add dependency to system failed! The source system is not in stage");
        let src_id = TypeId::of::<Src>();
        let dep_id = TypeId::of::<Dep>();
        let dependencies = &mut self.systems.get_mut(&src_id)
            .unwrap()
            .dependencies;
        debug_assert!(!dependencies.contains(&dep_id),
            "Add dependency to system failed! The dependency system has already in source system");
        dependencies.push(dep_id)
    }

    /// Dynamically remove a dependency from system
    /// ### Panics
    /// * Panic if the source system is not in stage
    /// * Panic if there is no dependency system in the source system
    pub fn remove_dependency<Src,Dep>(&mut self)
        where Src : for<'a> System<'a>,
              Dep : for<'a> System<'a> {
        debug_assert!(self.has_system::<Src>(),
            "Remove dependency from system failed! The source system is not in stage");
        let src_id = TypeId::of::<Src>();
        let dep_id = TypeId::of::<Dep>();
        let dependencies = &mut self.systems.get_mut(&src_id)
            .unwrap()
            .dependencies;
        let index = dependencies.iter()
            .enumerate()
            .find(|(_,type_id)|{
                **type_id == dep_id
            }).map(|(index,_)|index);
        debug_assert!(index.is_some(),
            "Remove dependency from system failed! Cannot find dependency system");
        let index = index.unwrap();
        dependencies.remove(index);
    }

    /// Execute all systems in stage.
    /// ### Details
    /// * Once Systems will be removed after ran.
    /// * System will be ran with topological order
    pub fn run(&mut self) {
        self.update();
        // initialize all systems
        for system_type in self.need_init.iter().cloned() {
            self.systems
                .get(&system_type)
                .unwrap()
                .system
                .borrow_mut()
                .initialize(self);
        }
        self.need_init.clear();
        let mut remove_list = vec![];
        for type_id in &self.run_queue {
            let system = self.systems
                .get(type_id)
                .unwrap();
            if system.is_active {
                if system.is_once {
                    remove_list.push(*type_id);
                }
                system.system.borrow_mut().run(self);
            }
        }
        for type_id in remove_list {
            self.systems.remove(&type_id);
        }
    }

    // update a run queue
    fn update(&mut self) {
        if !self.need_update {
            return;
        }
        self.run_queue.clear();
        let mut inverse_map = HashMap::new();
        let mut enter_edges_count = HashMap::new();
        // initialization
        for (type_id,system_info) in &self.systems {
            inverse_map.insert(*type_id,vec![]);
            enter_edges_count.insert(*type_id,system_info.dependencies.len());
        }
        inverse_map.insert(TypeId::of::<End>(),vec![]);
        // build inverse map
        for (self_type,self_system_info) in &self.systems {
            for dep_sys in &self_system_info.dependencies {
                inverse_map.get_mut(dep_sys)
                    .expect("Some dependencies have not been added to stage")
                    .push(*self_type)
            }
        }
        // topological sort
        fn find_zero(map : &HashMap<TypeId,usize>) -> Option<TypeId> {
            for (type_id,count) in map {
                // ignore the End
                if *type_id == TypeId::of::<End>() {
                    continue
                }
                if *count == 0 {
                    return Some(*type_id);
                }
            }
            None
        }
        fn sort(inverse_map : &HashMap<TypeId,Vec<TypeId>>,
                enter_edges_count : &mut HashMap<TypeId,usize>,
                run_queue : &mut Vec<TypeId>) {
            while let Some(type_id) = find_zero(enter_edges_count) {
                enter_edges_count.remove(&type_id);
                run_queue.push(type_id);
                for system in inverse_map.get(&type_id).unwrap().iter() {
                    let count = enter_edges_count.get_mut(system).unwrap();
                    *count -= 1;
                }
            }
        }
        sort(&inverse_map,&mut enter_edges_count,&mut self.run_queue);
        // sort remain systems
        if let Some(systems) = inverse_map.get(&TypeId::of::<End>()) {
            for system in systems.iter() {
                let count = enter_edges_count.get_mut(system).unwrap();
                *count -= 1;
            }
            sort(&inverse_map, &mut enter_edges_count, &mut self.run_queue);
        }
    }

}

#[cfg(test)]
mod tests{
    use crate::World;
    use crate::stage::Stage;
    use crate::system::{System, End, Errors};
    use crate::resource::Resource;
    use std::convert::Infallible;
    use std::fmt::{Display, Formatter};
    use std::error::Error;
    use std::cell::{Ref, RefMut};

    #[test]
    fn test_run() {
        let mut world = World::new();

        world.register::<char>();

        world.create_entity().attach('c');
        world.create_entity().attach('a');
        world.create_entity().attach('f');

        let mut stage = Stage::from_world(world);

        struct StartSystem;
        struct PrintSystem;
        #[derive(Debug)]
        struct DataSystemName(String);
        #[derive(Debug)]
        struct DataSystemAge(u32);
        struct AfterAll;
        struct LastOfEnd;

        impl<'a> System<'a> for StartSystem {
            type InitResource = ();
            type Resource = ();
            type Dependencies = ();
            type Error = Infallible;

            fn update(&'a mut self, _resource: <Self::Resource as Resource<'a>>::Type) -> Result<(),Infallible>{
                println!("Start");
                Ok(())
            }
        }

        impl<'a> System<'a> for PrintSystem {
            type InitResource = ();
            type Resource = (&'a World,&'a DataSystemName,&'a mut DataSystemAge);
            type Dependencies = StartSystem;
            type Error = Infallible;

            fn update(&mut self, (world,name,age) : <Self::Resource as Resource<'a>>::Type) -> Result<(),Infallible> {
                let v = world.query::<&char>().cloned().collect::<Vec<_>>();
                dbg!(&v);
                dbg!(&name.0);
                dbg!(&age.0);
                Ok(())
            }
        }

        impl<'a> System<'a> for DataSystemName{
            type InitResource = ();
            type Resource = ();
            type Dependencies = StartSystem;
            type Error = Infallible;

            fn init(&'a mut self, _ : ()) -> Result<(),Infallible> {
                println!("DataSystemName has been added to stage");
                Ok(())
            }
        }
        impl<'a> System<'a> for DataSystemAge {
            type InitResource = ();
            type Resource = ();
            type Dependencies = StartSystem;
            type Error = Infallible;

            fn init(&'a mut self, _ : ()) -> Result<(),Infallible>{
                println!("DataSystemAge has been added to stage");
                Ok(())
            }
        }

        impl<'a> System<'a> for AfterAll {
            type InitResource = ();
            type Resource = ();
            type Dependencies = End;
            type Error = Infallible;

            fn update(&'a mut self, _resource: <Self::Resource as Resource<'a>>::Type) -> Result<(),Infallible>{
                println!("Finished");
                Ok(())
            }
        }
        impl<'a> System<'a> for LastOfEnd {
            type InitResource = ();
            type Resource = ();
            type Dependencies = End;
            type Error = Infallible;

            fn update(&'a mut self, _resource: <Self::Resource as Resource<'a>>::Type) -> Result<(),Infallible>{
                println!("Finished!!!");
                Ok(())
            }
        }

        stage
            .add_system(StartSystem)
            .add_system(PrintSystem)
            .add_system(DataSystemName("asda".to_string()))
            .add_system(DataSystemAge(13))
            .add_system(AfterAll)
            .add_system(LastOfEnd);

        stage.run();

        stage.run();

        stage.deactivate::<PrintSystem>();

        stage.run();
        stage.add_dependency::<DataSystemAge,DataSystemName>();

        stage.activate::<PrintSystem>();
        stage.run();
        stage.remove_dependency::<DataSystemAge,DataSystemName>();
    }

    #[test]
    fn error_test(){
        let mut stage = Stage::new();

        #[derive(Debug)]
        struct MyError(i32);
        impl Display for MyError {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f,"Error with value : {}",self.0)
            }
        }
        impl Error for MyError{}

        struct ErrorSource{
            count : i32
        }
        impl<'a> System<'a> for ErrorSource {
            type InitResource = ();
            type Resource = ();
            type Dependencies = ();
            type Error = MyError;

            fn update(&'a mut self, _resource: <Self::Resource as Resource<'a>>::Type) -> Result<(), Self::Error> {
                let err = self.count;
                self.count += 1;
                Err(MyError(err))
            }
        }

        struct ErrorHandler;
        impl<'a> System<'a> for ErrorHandler {
            type InitResource = ();
            type Resource = &'a mut Errors;
            type Dependencies = ErrorSource;
            type Error = Infallible;

            fn update(&'a mut self,mut errors : RefMut<'a,Errors>) -> Result<(), Self::Error> {
                if let Some(error) = errors.fetch_error::<ErrorSource>() {
                    println!("Catch error with value {}",error.as_ref().0);
                }
                // for error in errors.fetch_all_errors() {
                    // println!("{}",error);
                // }
                Ok(())
            }
        }

        stage.add_system(ErrorSource{count : 1})
            .add_system(ErrorHandler);

        stage.run();
        stage.run();
    }

    #[test]
    fn init_test() {
        let mut stage = Stage::new();

        struct Data {
            fuck : i32
        }
        impl<'a> System<'a> for Data {
            type InitResource = ();
            type Resource = ();
            type Dependencies = ();
            type Error = Infallible;
        }

        struct PrintDataInInit;
        impl<'a> System<'a> for PrintDataInInit {
            type InitResource = &'a Data;
            type Resource = ();
            type Dependencies = ();
            type Error = Infallible;

            fn init(&'a mut self, resource: <Self::InitResource as Resource<'a>>::Type) -> Result<(), Self::Error> {
                let data: Ref<'a,Data> = resource;
                println!("fuck u:{}",data.fuck);
                Ok(())
            }
        }

        stage.add_system(Data{
            fuck : 3
        }).add_system(PrintDataInInit);

        stage.run();
    }
}
