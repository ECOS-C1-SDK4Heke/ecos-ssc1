# ECOS-Macros

需要增加其他外设的默认初始化/取消初始化，只需要：

- ecos_main 增加宏标签注释以说明
- 手动在属性宏内部 pm.register 以及 pm.add_preset
- 增加标签的行为映射在 fn process_option 内

> todo-list: 未来将上面仨也封装为一个宏懒省事...

其中：

注册到off的都是默认会初始化的，注册到on的都是默认不会初始化的

若需要禁用默认初始化的就直接：#[ecos_main(no_选项)]

若需要开启默认不会初始化就是：#[ecos_main(选项)]

可以一键开启所有的：#[ecos_main(on)]

可以一键禁用所有的：#[ecos_main(off)] 或者直接 #[rust_main]

其中，on会开启所有注册到on的，由于没有注册到on的都是会默认初始化的，所以on也就是开启了所有默认初始化

其中，off会禁用所有注册到off的，由于没有注册到off的都是不会默认初始化的，所以off也就是关闭了所有默认初始化
